// SPDX-License-Identifier: MIT OR Apache-2.0
//
// tgui Android notification bridge.
//
// Compiled to bridge.dex and loaded at runtime via InMemoryDexClassLoader.
package com.tgui;

import android.Manifest;
import android.app.Activity;
import android.app.Fragment;
import android.app.FragmentTransaction;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;

public final class TguiNotificationBridge {
    public static final int PERMISSION_NOT_DETERMINED = 0;
    public static final int PERMISSION_GRANTED = 1;
    public static final int PERMISSION_DENIED = 2;

    private static final String ACTION_NOTIFICATION = "com.tgui.NOTIFICATION_ACTION";
    private static final String EXTRA_CALLBACK_ID = "callbackId";
    private static final String EXTRA_ACTION_ID = "actionId";
    private static final String PREFS_NAME = "tgui_notifications";
    private static final String PREF_POST_NOTIFICATIONS_REQUESTED = "post_notifications_requested";

    private static ActionReceiver actionReceiver;

    private TguiNotificationBridge() {}

    public static void install(final Activity activity) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                ensureActionReceiver(activity);
            }
        });
    }

    public static void sendNotification(
            final Activity activity,
            final String appId,
            final String notificationId,
            final String channelName,
            final String title,
            final String body,
            final String subtitle,
            final String iconName,
            final boolean sound,
            final long actionCallbackId,
            final String[] actionIds,
            final String[] actionLabels) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                ensureActionReceiver(activity);

                NotificationManager manager =
                        (NotificationManager) activity.getSystemService(Context.NOTIFICATION_SERVICE);
                if (manager == null) {
                    return;
                }

                Notification.Builder builder;
                if (Build.VERSION.SDK_INT >= 26) {
                    String channelId = ensureChannel(manager, appId, channelName, sound);
                    builder = new Notification.Builder(activity, channelId);
                } else {
                    builder = new Notification.Builder(activity);
                    if (sound) {
                        builder.setDefaults(Notification.DEFAULT_SOUND);
                    } else {
                        builder.setDefaults(0);
                        builder.setSound(null);
                    }
                }

                builder.setSmallIcon(resolveSmallIcon(activity, iconName));
                builder.setContentTitle(title);
                builder.setAutoCancel(true);
                builder.setWhen(System.currentTimeMillis());
                if (body != null) {
                    builder.setContentText(body);
                    builder.setStyle(new Notification.BigTextStyle().bigText(body));
                } else if (subtitle != null) {
                    builder.setContentText(subtitle);
                }
                if (subtitle != null) {
                    builder.setSubText(subtitle);
                } else if (channelName != null && !channelName.isEmpty()) {
                    builder.setSubText(channelName);
                }

                Intent launchIntent =
                        activity.getPackageManager().getLaunchIntentForPackage(activity.getPackageName());
                if (launchIntent != null) {
                    launchIntent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_CLEAR_TOP);
                    builder.setContentIntent(
                            PendingIntent.getActivity(
                                    activity,
                                    requestCode(notificationId, 0),
                                    launchIntent,
                                    pendingIntentFlags()));
                }

                if (actionCallbackId > 0L && actionIds != null && actionLabels != null) {
                    int count = Math.min(actionIds.length, actionLabels.length);
                    for (int i = 0; i < count; i++) {
                        Intent actionIntent = new Intent(ACTION_NOTIFICATION);
                        actionIntent.setPackage(activity.getPackageName());
                        actionIntent.putExtra(EXTRA_CALLBACK_ID, actionCallbackId);
                        actionIntent.putExtra(EXTRA_ACTION_ID, actionIds[i]);
                        PendingIntent pendingIntent =
                                PendingIntent.getBroadcast(
                                        activity,
                                        requestCode(notificationId, i + 1),
                                        actionIntent,
                                        pendingIntentFlags());
                        Notification.Action action =
                                new Notification.Action.Builder(0, actionLabels[i], pendingIntent).build();
                        builder.addAction(action);
                    }
                }

                manager.notify(notificationId, 0, builder.build());
            }
        });
    }

    public static void requestPermission(final Activity activity, final long requestId) {
        if (Build.VERSION.SDK_INT < 33) {
            onPermissionResult(requestId, PERMISSION_GRANTED);
            return;
        }
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                        == PackageManager.PERMISSION_GRANTED) {
                    onPermissionResult(requestId, PERMISSION_GRANTED);
                    return;
                }

                activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                        .edit()
                        .putBoolean(PREF_POST_NOTIFICATIONS_REQUESTED, true)
                        .apply();

                PermissionFragment fragment = new PermissionFragment();
                Bundle args = new Bundle();
                args.putLong("requestId", requestId);
                fragment.setArguments(args);
                FragmentTransaction tx = activity.getFragmentManager().beginTransaction();
                tx.add(fragment, "TguiNotificationPermission#" + requestId);
                tx.commitAllowingStateLoss();
            }
        });
    }

    public static int permissionStatus(Activity activity) {
        if (Build.VERSION.SDK_INT < 33) {
            return PERMISSION_GRANTED;
        }
        if (activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                == PackageManager.PERMISSION_GRANTED) {
            return PERMISSION_GRANTED;
        }
        boolean requested =
                activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                        .getBoolean(PREF_POST_NOTIFICATIONS_REQUESTED, false);
        return requested ? PERMISSION_DENIED : PERMISSION_NOT_DETERMINED;
    }

    private static void ensureActionReceiver(Activity activity) {
        if (actionReceiver != null) {
            return;
        }
        actionReceiver = new ActionReceiver();
        IntentFilter filter = new IntentFilter(ACTION_NOTIFICATION);
        if (Build.VERSION.SDK_INT >= 33) {
            activity.registerReceiver(actionReceiver, filter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            activity.registerReceiver(actionReceiver, filter);
        }
    }

    private static String ensureChannel(
            NotificationManager manager,
            String appId,
            String channelName,
            boolean sound) {
        String baseName = (channelName == null || channelName.isEmpty()) ? appId : channelName;
        String channelId = appId + (sound ? ".default" : ".silent");
        int importance = sound ? NotificationManager.IMPORTANCE_DEFAULT : NotificationManager.IMPORTANCE_LOW;
        NotificationChannel channel =
                new NotificationChannel(
                        channelId,
                        sound ? baseName : baseName + " (silent)",
                        importance);
        if (!sound) {
            channel.setSound(null, null);
            channel.enableVibration(false);
        }
        manager.createNotificationChannel(channel);
        return channelId;
    }

    private static int resolveSmallIcon(Activity activity, String iconName) {
        if (iconName != null && !iconName.isEmpty()) {
            int resolved = activity.getResources().getIdentifier(iconName, "drawable", activity.getPackageName());
            if (resolved == 0) {
                resolved = activity.getResources().getIdentifier(iconName, "mipmap", activity.getPackageName());
            }
            if (resolved != 0) {
                return resolved;
            }
        }

        int applicationIcon = activity.getApplicationInfo().icon;
        if (applicationIcon != 0) {
            return applicationIcon;
        }
        return android.R.drawable.ic_dialog_info;
    }

    private static int pendingIntentFlags() {
        return PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE;
    }

    private static int requestCode(String notificationId, int salt) {
        int hash = notificationId != null ? notificationId.hashCode() : 0;
        return (0x41F00000 ^ hash ^ salt) & 0x7fffffff;
    }

    private static native void onNotificationAction(long callbackId, String actionId);

    private static native void onPermissionResult(long requestId, int status);

    public static final class ActionReceiver extends BroadcastReceiver {
        @Override
        public void onReceive(Context context, Intent intent) {
            if (intent == null) {
                return;
            }
            long callbackId = intent.getLongExtra(EXTRA_CALLBACK_ID, -1L);
            String actionId = intent.getStringExtra(EXTRA_ACTION_ID);
            if (callbackId >= 0L && actionId != null) {
                onNotificationAction(callbackId, actionId);
            }
        }
    }

    public static final class PermissionFragment extends Fragment {
        private static final int REQUEST_CODE = 0x7492;
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
            requestPermissions(new String[] {Manifest.permission.POST_NOTIFICATIONS}, REQUEST_CODE);
        }

        @Override
        public void onRequestPermissionsResult(
                int requestCode, String[] permissions, int[] grantResults) {
            super.onRequestPermissionsResult(requestCode, permissions, grantResults);
            if (requestCode != REQUEST_CODE) {
                return;
            }
            Bundle args = getArguments();
            long requestId = args != null ? args.getLong("requestId", -1L) : -1L;
            int status =
                    (grantResults != null
                                    && grantResults.length > 0
                                    && grantResults[0] == PackageManager.PERMISSION_GRANTED)
                            ? PERMISSION_GRANTED
                            : PERMISSION_DENIED;
            if (requestId >= 0L) {
                onPermissionResult(requestId, status);
            }
            detachSelf();
        }

        private void detachSelf() {
            if (getFragmentManager() != null) {
                getFragmentManager().beginTransaction().remove(this).commitAllowingStateLoss();
            }
        }
    }
}
