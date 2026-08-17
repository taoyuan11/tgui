//! UI-thread state and worker-message contracts.
//!
//! P0 does not yet implement reactive signals. It does establish the ownership
//! boundary: mutable transactions are non-`Send`, while background work can
//! only return generation/revision-stamped messages through `UiDispatcher`.

use crate::core::{Error, GenerationStamp, Result, RevisionSet, WindowId};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, ThreadId};

/// A non-`Send` token identifying the UI thread that owns a tree or transaction.
#[derive(Clone, Debug)]
pub struct UiThread {
    id: ThreadId,
    // Rc intentionally makes the token (and types containing it) !Send + !Sync.
    _not_send: PhantomData<Rc<()>>,
}

impl UiThread {
    pub fn current() -> Self {
        Self {
            id: thread::current().id(),
            _not_send: PhantomData,
        }
    }

    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub fn is_current(&self) -> bool {
        thread::current().id() == self.id
    }

    pub fn assert_current(&self) -> Result<()> {
        if self.is_current() {
            Ok(())
        } else {
            Err(Error::platform(
                "ui_thread",
                "operation attempted from a non-owner thread",
                false,
            ))
        }
    }
}

/// One worker result with the identity and revisions observed at request time.
#[derive(Debug)]
pub struct BackgroundMessage<T> {
    pub target: WindowId,
    pub source: GenerationStamp,
    pub requested_revisions: RevisionSet,
    pub payload: T,
}

impl<T> BackgroundMessage<T> {
    pub fn new(
        target: WindowId,
        source: GenerationStamp,
        requested_revisions: RevisionSet,
        payload: T,
    ) -> Self {
        Self {
            target,
            source,
            requested_revisions,
            payload,
        }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> BackgroundMessage<U> {
        BackgroundMessage {
            target: self.target,
            source: self.source,
            requested_revisions: self.requested_revisions,
            payload: map(self.payload),
        }
    }
}

/// Cross-thread sender. It has no reference to an application or its arena.
#[derive(Clone, Debug)]
pub struct UiDispatcher<T> {
    sender: Sender<BackgroundMessage<T>>,
}

impl<T: Send + 'static> UiDispatcher<T> {
    pub fn send(&self, message: BackgroundMessage<T>) -> Result<()> {
        self.sender
            .send(message)
            .map_err(|_| Error::platform("ui_dispatch", "the UI receiver has been dropped", true))
    }

    pub fn dispatch(
        &self,
        target: WindowId,
        source: GenerationStamp,
        requested_revisions: RevisionSet,
        payload: T,
    ) -> Result<()> {
        self.send(BackgroundMessage::new(
            target,
            source,
            requested_revisions,
            payload,
        ))
    }
}

/// UI-owned receiver for worker results.
pub struct UiInbox<T> {
    receiver: Receiver<BackgroundMessage<T>>,
    owner: UiThread,
}

impl<T> UiInbox<T> {
    pub fn try_recv(&self) -> Result<Option<BackgroundMessage<T>>> {
        self.owner.assert_current()?;
        match self.receiver.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Ok(None),
        }
    }

    pub fn drain(&self) -> Result<Vec<BackgroundMessage<T>>> {
        self.owner.assert_current()?;
        let mut messages = Vec::new();
        while let Ok(message) = self.receiver.try_recv() {
            messages.push(message);
        }
        Ok(messages)
    }

    /// Accepts only messages whose generation/revision metadata is still valid.
    pub fn drain_valid(
        &self,
        mut is_current: impl FnMut(&BackgroundMessage<T>) -> bool,
    ) -> Result<DispatchBatch<T>> {
        self.owner.assert_current()?;
        let mut accepted = Vec::new();
        let mut stale = 0;
        loop {
            match self.receiver.try_recv() {
                Ok(message) if is_current(&message) => accepted.push(message),
                Ok(_) => stale += 1,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        Ok(DispatchBatch { accepted, stale })
    }
}

/// Result of a validated inbox drain.
#[derive(Debug)]
pub struct DispatchBatch<T> {
    pub accepted: Vec<BackgroundMessage<T>>,
    pub stale: usize,
}

/// Creates the only supported worker-to-UI channel.
pub fn ui_channel<T: Send + 'static>() -> (UiDispatcher<T>, UiInbox<T>) {
    let (sender, receiver) = mpsc::channel();
    (
        UiDispatcher { sender },
        UiInbox {
            receiver,
            owner: UiThread::current(),
        },
    )
}

/// Small command vocabulary used by the P0 transaction contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiCommand {
    RequestFrame(WindowId),
    CloseWindow(WindowId),
}

/// UI-only batch of mutations. The closure receives all commands at once so a
/// caller can apply or reject the batch atomically.
pub struct UpdateTxn<C = UiCommand> {
    owner: UiThread,
    commands: Vec<C>,
    committed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxnReceipt {
    pub command_count: usize,
}

impl<C> UpdateTxn<C> {
    pub fn new() -> Self {
        Self {
            owner: UiThread::current(),
            commands: Vec::new(),
            committed: false,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            owner: UiThread::current(),
            commands: Vec::with_capacity(capacity),
            committed: false,
        }
    }

    pub fn push(&mut self, command: C) -> Result<()> {
        self.owner.assert_current()?;
        if self.committed {
            return Err(Error::platform(
                "update_txn",
                "cannot append to a committed transaction",
                false,
            ));
        }
        self.commands.push(command);
        Ok(())
    }

    pub fn extend(&mut self, commands: impl IntoIterator<Item = C>) -> Result<()> {
        self.owner.assert_current()?;
        if self.committed {
            return Err(Error::platform(
                "update_txn",
                "cannot append to a committed transaction",
                false,
            ));
        }
        self.commands.extend(commands);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn commands(&self) -> &[C] {
        &self.commands
    }

    pub fn into_commands(self) -> Vec<C> {
        self.commands
    }

    pub fn rollback(mut self) -> Vec<C> {
        self.committed = true;
        self.commands
    }

    pub fn commit(mut self, apply: impl FnOnce(Vec<C>) -> Result<()>) -> Result<TxnReceipt> {
        self.owner.assert_current()?;
        if self.committed {
            return Err(Error::platform(
                "update_txn",
                "transaction was already finalized",
                false,
            ));
        }
        let command_count = self.commands.len();
        // Move the complete batch into one callback: no half-applied command
        // sequence is created by this contract.
        let commands = std::mem::take(&mut self.commands);
        apply(commands)?;
        self.committed = true;
        Ok(TxnReceipt { command_count })
    }
}

impl<C> Default for UpdateTxn<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ElementId;

    #[test]
    fn transactions_apply_one_coalesced_batch() {
        let mut txn = UpdateTxn::<u32>::new();
        txn.push(1).unwrap();
        txn.push(2).unwrap();
        let mut seen = Vec::new();
        let receipt = txn.commit(|commands| {
            seen.extend(commands);
            Ok(())
        });

        assert_eq!(receipt.unwrap().command_count, 2);
        assert_eq!(seen, [1, 2]);
    }

    #[test]
    fn worker_messages_are_generation_and_revision_stamped() {
        let (dispatcher, inbox) = ui_channel::<u32>();
        let id = ElementId::from_parts(4, 9);
        let revisions = RevisionSet::ZERO;
        dispatcher
            .dispatch(WindowId::from_parts(0, 1), id.stamp(), revisions, 42)
            .unwrap();
        let batch = inbox
            .drain_valid(|message| message.source.matches(id))
            .unwrap();
        assert_eq!(batch.stale, 0);
        assert_eq!(batch.accepted[0].payload, 42);
        assert_eq!(batch.accepted[0].requested_revisions, revisions);
    }
}
