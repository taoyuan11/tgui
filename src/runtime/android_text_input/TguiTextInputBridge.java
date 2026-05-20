// SPDX-License-Identifier: MIT OR Apache-2.0
package com.tgui;

import android.app.Activity;
import android.content.Context;
import android.text.Editable;
import android.text.InputType;
import android.text.Selection;
import android.text.TextWatcher;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.FrameLayout;

public final class TguiTextInputBridge {
    private static EditText inputView;
    private static boolean suppressCallbacks;

    private TguiTextInputBridge() {}

    public static void install(final Activity activity) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (inputView != null) {
                    return;
                }

                final EditText view = new EditText(activity);
                view.setLayoutParams(new FrameLayout.LayoutParams(1, 1));
                view.setAlpha(0f);
                view.setSingleLine(false);
                view.setCursorVisible(false);
                view.setFocusable(true);
                view.setFocusableInTouchMode(true);
                view.setLongClickable(false);
                view.setHorizontallyScrolling(false);
                view.setTextIsSelectable(false);
                view.setInputType(
                        InputType.TYPE_CLASS_TEXT
                                | InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
                                | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
                view.setImeOptions(
                        EditorInfo.IME_FLAG_NO_EXTRACT_UI
                                | EditorInfo.IME_FLAG_NO_FULLSCREEN
                                | EditorInfo.IME_FLAG_NO_ENTER_ACTION);
                view.addTextChangedListener(new TextWatcher() {
                    @Override public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
                    @Override public void onTextChanged(CharSequence s, int start, int before, int count) {}
                    @Override public void afterTextChanged(Editable editable) {
                        if (suppressCallbacks) {
                            return;
                        }
                        dispatchChange(view, editable);
                    }
                });

                FrameLayout root = activity.findViewById(android.R.id.content);
                root.addView(view);
                inputView = view;
            }
        });
    }

    public static void setInputEnabled(final Activity activity, final boolean enabled) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (inputView == null) {
                    return;
                }
                InputMethodManager imm = (InputMethodManager) activity.getSystemService(Context.INPUT_METHOD_SERVICE);
                if (enabled) {
                    inputView.requestFocus();
                    if (imm != null) {
                        imm.showSoftInput(inputView, InputMethodManager.SHOW_IMPLICIT);
                    }
                } else {
                    if (imm != null) {
                        imm.hideSoftInputFromWindow(inputView.getWindowToken(), 0);
                    }
                    inputView.clearFocus();
                }
            }
        });
    }

    public static void syncState(
            final Activity activity,
            final String text,
            final int selectionStart,
            final int selectionEnd,
            final int composingStart,
            final int composingEnd) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (inputView == null) {
                    return;
                }
                suppressCallbacks = true;
                try {
                    Editable editable = inputView.getText();
                    editable.replace(0, editable.length(), text);
                    BaseInputConnection.removeComposingSpans(editable);
                    BaseInputConnection connection = new BaseInputConnection(inputView, true);
                    if (composingStart >= 0 && composingEnd >= composingStart && composingEnd <= editable.length()) {
                        Selection.setSelection(editable, composingStart, composingEnd);
                        connection.setComposingRegion(composingStart, composingEnd);
                    }
                    int start = clamp(editable.length(), selectionStart);
                    int end = clamp(editable.length(), selectionEnd);
                    Selection.setSelection(editable, start, end);
                } finally {
                    suppressCallbacks = false;
                }
            }
        });
    }

    private static void dispatchChange(EditText view, Editable editable) {
        int selectionStart = Selection.getSelectionStart(editable);
        int selectionEnd = Selection.getSelectionEnd(editable);
        int composingStart = BaseInputConnection.getComposingSpanStart(editable);
        int composingEnd = BaseInputConnection.getComposingSpanEnd(editable);
        onTextChanged(
                editable.toString(),
                selectionStart < 0 ? 0 : selectionStart,
                selectionEnd < 0 ? 0 : selectionEnd,
                composingStart,
                composingEnd);
    }

    private static int clamp(int len, int value) {
        if (value < 0) {
            return 0;
        }
        return Math.min(len, value);
    }

    private static native void onTextChanged(
            String text,
            int selectionStart,
            int selectionEnd,
            int composingStart,
            int composingEnd);
}
