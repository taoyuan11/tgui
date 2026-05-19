// SPDX-License-Identifier: MIT OR Apache-2.0
//
// tgui Android dialog bridge.
//
// Compiled to bridge.dex and loaded at runtime via InMemoryDexClassLoader.
// See build_bridge.md for the regeneration command.
//
// API level baseline: 26 (uses InMemoryDexClassLoader on the Rust side and
// runOnUiThread / Activity.startActivityForResult here).
package com.tgui;

import android.app.Activity;
import android.app.AlertDialog;
import android.app.Fragment;
import android.app.FragmentTransaction;
import android.content.ClipData;
import android.content.Intent;
import android.content.DialogInterface;
import android.net.Uri;
import android.os.Bundle;
import java.util.ArrayList;

public final class TguiDialogBridge {
    // Button identifiers shared with Rust (mirror MessageDialogResult ordering).
    public static final int BUTTON_OK = 1;
    public static final int BUTTON_CANCEL = 2;
    public static final int BUTTON_YES = 3;
    public static final int BUTTON_NO = 4;

    // Button layouts mirror MessageDialogButtons.
    public static final int BUTTONS_OK = 0;
    public static final int BUTTONS_OK_CANCEL = 1;
    public static final int BUTTONS_YES_NO = 2;
    public static final int BUTTONS_YES_NO_CANCEL = 3;

    // File request kinds mirror FileDialogRequest.
    public static final int FILE_OPEN = 0;
    public static final int FILE_OPEN_MULTI = 1;
    public static final int FILE_PICK_FOLDER = 2;
    public static final int FILE_SAVE = 3;

    private TguiDialogBridge() {}

    public static void showMessageDialog(
            final Activity activity,
            final long requestId,
            final String title,
            final String message,
            final int buttonsKind) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                AlertDialog.Builder builder = new AlertDialog.Builder(activity);
                if (title != null) {
                    builder.setTitle(title);
                }
                if (message != null) {
                    builder.setMessage(message);
                }
                switch (buttonsKind) {
                    case BUTTONS_OK_CANCEL:
                        builder.setPositiveButton(android.R.string.ok, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_OK); }
                        });
                        builder.setNegativeButton(android.R.string.cancel, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_CANCEL); }
                        });
                        break;
                    case BUTTONS_YES_NO:
                        builder.setPositiveButton(android.R.string.yes, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_YES); }
                        });
                        builder.setNegativeButton(android.R.string.no, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_NO); }
                        });
                        break;
                    case BUTTONS_YES_NO_CANCEL:
                        builder.setPositiveButton(android.R.string.yes, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_YES); }
                        });
                        builder.setNegativeButton(android.R.string.no, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_NO); }
                        });
                        builder.setNeutralButton(android.R.string.cancel, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_CANCEL); }
                        });
                        break;
                    case BUTTONS_OK:
                    default:
                        builder.setPositiveButton(android.R.string.ok, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int w) { onMessageResult(requestId, BUTTON_OK); }
                        });
                        break;
                }
                builder.setOnCancelListener(new DialogInterface.OnCancelListener() {
                    @Override public void onCancel(DialogInterface d) { onMessageResult(requestId, BUTTON_CANCEL); }
                });
                builder.show();
            }
        });
    }

    public static void startFileDialog(
            final Activity activity,
            final long requestId,
            final int requestKind,
            final String title,
            final String suggestedFileName,
            final String[] mimeTypes) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Intent intent = buildIntent(requestKind, suggestedFileName, mimeTypes);
                if (title != null) {
                    intent = Intent.createChooser(intent, title);
                }
                ResultFragment fragment = new ResultFragment();
                Bundle args = new Bundle();
                args.putLong("requestId", requestId);
                args.putParcelable("intent", intent);
                fragment.setArguments(args);
                FragmentTransaction tx = activity.getFragmentManager().beginTransaction();
                tx.add(fragment, "TguiDialogBridge#" + requestId);
                tx.commitAllowingStateLoss();
            }
        });
    }

    private static Intent buildIntent(int requestKind, String suggestedFileName, String[] mimeTypes) {
        Intent intent;
        switch (requestKind) {
            case FILE_PICK_FOLDER:
                intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
                break;
            case FILE_SAVE:
                intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                if (suggestedFileName != null) {
                    intent.putExtra(Intent.EXTRA_TITLE, suggestedFileName);
                }
                intent.setType(primaryMime(mimeTypes));
                break;
            case FILE_OPEN_MULTI:
                intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                applyMimeTypes(intent, mimeTypes);
                break;
            case FILE_OPEN:
            default:
                intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                applyMimeTypes(intent, mimeTypes);
                break;
        }
        return intent;
    }

    private static String primaryMime(String[] mimeTypes) {
        if (mimeTypes != null && mimeTypes.length > 0) {
            return mimeTypes[0];
        }
        return "*/*";
    }

    private static void applyMimeTypes(Intent intent, String[] mimeTypes) {
        if (mimeTypes == null || mimeTypes.length == 0) {
            intent.setType("*/*");
        } else if (mimeTypes.length == 1) {
            intent.setType(mimeTypes[0]);
        } else {
            intent.setType("*/*");
            intent.putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes);
        }
    }

    private static native void onMessageResult(long requestId, int which);
    private static native void onFileResult(long requestId, int resultCode, String[] uris);

    public static final class ResultFragment extends Fragment {
        private static final int REQUEST_CODE = 0x7491;
        private boolean launched;

        @Override
        public void onCreate(Bundle savedInstanceState) {
            super.onCreate(savedInstanceState);
            setRetainInstance(true);
        }

        @Override
        public void onActivityCreated(Bundle savedInstanceState) {
            super.onActivityCreated(savedInstanceState);
            if (launched) {
                return;
            }
            launched = true;
            Bundle args = getArguments();
            if (args == null) {
                return;
            }
            Intent intent = args.getParcelable("intent");
            if (intent == null) {
                long failedId = args.getLong("requestId", -1L);
                if (failedId >= 0) {
                    onFileResult(failedId, Activity.RESULT_CANCELED, new String[0]);
                }
                detachSelf();
                return;
            }
            startActivityForResult(intent, REQUEST_CODE);
        }

        @Override
        public void onActivityResult(int requestCode, int resultCode, Intent data) {
            super.onActivityResult(requestCode, resultCode, data);
            if (requestCode != REQUEST_CODE) {
                return;
            }
            Bundle args = getArguments();
            long requestId = args != null ? args.getLong("requestId", -1L) : -1L;
            String[] uris = extractUris(data);
            if (requestId >= 0) {
                onFileResult(requestId, resultCode, uris);
            }
            detachSelf();
        }

        private void detachSelf() {
            if (getFragmentManager() != null) {
                getFragmentManager().beginTransaction().remove(this).commitAllowingStateLoss();
            }
        }

        private static String[] extractUris(Intent data) {
            if (data == null) {
                return new String[0];
            }
            ArrayList<String> collected = new ArrayList<>();
            Uri single = data.getData();
            if (single != null) {
                collected.add(single.toString());
            }
            ClipData clip = data.getClipData();
            if (clip != null) {
                for (int i = 0; i < clip.getItemCount(); i++) {
                    Uri uri = clip.getItemAt(i).getUri();
                    if (uri != null) {
                        String s = uri.toString();
                        if (!collected.contains(s)) {
                            collected.add(s);
                        }
                    }
                }
            }
            return collected.toArray(new String[0]);
        }
    }
}
