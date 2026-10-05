// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
package org.openmobileemulator.ime;

import android.inputmethodservice.InputMethodService;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Handler;
import android.os.Looper;
import android.system.ErrnoException;
import android.system.Os;
import android.system.OsConstants;
import android.text.InputType;
import android.util.Log;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;
import android.view.inputmethod.InputConnection;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.Closeable;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.Semaphore;

public final class OmeInputMethodService extends InputMethodService {
    private static final String TAG = "OmeIme";
    private static final int MAX_FRAME_CHARS = 65536;
    private final Handler main = new Handler(Looper.getMainLooper());
    private final Object socketLock = new Object();
    private volatile boolean stopped;
    private LocalServerSocket server;
    private volatile Client client;
    private volatile long inputGeneration;
    // Editor state and Client.ready are accessed only on the main thread.
    private JSONObject focus;
    private String composition = "";
    private long versionCode;

    @Override
    public void onCreate() {
        super.onCreate();
        try {
            versionCode = getPackageManager().getPackageInfo(getPackageName(), 0).getLongVersionCode();
        } catch (android.content.pm.PackageManager.NameNotFoundException exception) {
            throw new IllegalStateException("IME package not found");
        }
        new Thread(this::acceptClients, "OmeIme-accept").start();
    }

    @Override
    public View onCreateInputView() {
        return null;
    }

    @Override
    public boolean onEvaluateInputViewShown() {
        return false;
    }

    @Override
    public boolean onEvaluateFullscreenMode() {
        return false;
    }

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {
        return super.onKeyDown(keyCode, event);
    }

    @Override
    public boolean onKeyUp(int keyCode, KeyEvent event) {
        return super.onKeyUp(keyCode, event);
    }

    @Override
    public void onStartInput(EditorInfo info, boolean restarting) {
        super.onStartInput(info, restarting);
        inputGeneration++;
        composition = "";
        focus = info.inputType == InputType.TYPE_NULL ? null : frame("focus",
                "inputType", info.inputType, "imeAction", info.imeOptions & EditorInfo.IME_MASK_ACTION,
                "package", info.packageName == null ? "" : info.packageName);
        sendState();
    }

    @Override
    public void onFinishInput() {
        inputGeneration++;
        focus = null;
        composition = "";
        sendState();
        super.onFinishInput();
    }

    private void sendState() {
        Client current = client;
        if (current != null && current.ready) {
            current.send(focus == null ? frame("blur") : focus);
        }
    }

    private void acceptClients() {
        try {
            synchronized (socketLock) {
                if (stopped) {
                    return;
                }
                server = new LocalServerSocket("ome-ime");
            }
            while (!stopped) {
                LocalSocket socket = server.accept();
                try {
                    // Abstract socket names have no filesystem permissions. Check SO_PEERCRED.
                    int uid = socket.getPeerCredentials().getUid();
                    if (uid != 0 && uid != 2000) {
                        closeQuietly(socket);
                        Log.w(TAG, "Rejected non-adb peer");
                        continue;
                    }
                    Client next = new Client(socket);
                    synchronized (socketLock) {
                        if (stopped) {
                            next.close();
                            return;
                        }
                        if (client != null) {
                            client.close();
                        }
                        client = next;
                    }
                    main.post(() -> {
                        if (client == next && !stopped) {
                            finishComposition();
                            next.send(frame("hello", "version", 1, "package", getPackageName(),
                                    "versionCode", versionCode));
                            next.ready = true;
                            if (focus != null) {
                                next.send(focus);
                            }
                        }
                    });
                    next.start();
                } catch (IOException exception) {
                    closeQuietly(socket);
                    Log.w(TAG, "Could not accept peer");
                }
            }
        } catch (IOException exception) {
            if (!stopped) {
                Log.w(TAG, "Socket listener stopped");
            }
        }
    }

    private void finishComposition() {
        if (!composition.isEmpty()) {
            InputConnection connection = getCurrentInputConnection();
            if (connection != null) {
                connection.finishComposingText();
            }
            composition = "";
        }
    }

    private void apply(JSONObject command) {
        InputConnection connection = getCurrentInputConnection();
        if (focus == null || connection == null) {
            return;
        }
        connection.beginBatchEdit();
        try {
            String op = command.optString("op");
            if ("compose".equals(op) || "commit".equals(op)) {
                Object value = command.opt("text");
                if (!(value instanceof String)) {
                    return;
                }
                String text = (String) value;
                if ("compose".equals(op)) {
                    setComposition(connection, text);
                } else {
                    connection.commitText(text, 1);
                    composition = "";
                }
                Log.d(TAG, op + " length=" + text.length());
            } else if ("key".equals(op)) {
                applyKey(connection, keyCode(command.optString("key")));
            }
        } finally {
            connection.endBatchEdit();
        }
    }

    private void setComposition(InputConnection connection, String text) {
        connection.setComposingText(text, 1);
        composition = text;
        if (text.isEmpty()) {
            connection.finishComposingText();
        }
    }

    private void applyKey(InputConnection connection, int code) {
        if (code == KeyEvent.KEYCODE_UNKNOWN) {
            return;
        }
        if (code == KeyEvent.KEYCODE_DEL || code == KeyEvent.KEYCODE_FORWARD_DEL) {
            deleteText(connection, code == KeyEvent.KEYCODE_DEL);
            return;
        }
        finishComposition();
        if (code == KeyEvent.KEYCODE_DPAD_LEFT || code == KeyEvent.KEYCODE_DPAD_RIGHT
                || code == KeyEvent.KEYCODE_MOVE_HOME || code == KeyEvent.KEYCODE_MOVE_END) {
            if (moveCursor(connection, code)) {
                return;
            }
        } else if (code == KeyEvent.KEYCODE_ENTER) {
            EditorInfo info = getCurrentInputEditorInfo();
            int action = info.imeOptions & EditorInfo.IME_MASK_ACTION;
            boolean multiline = (info.inputType & InputType.TYPE_MASK_CLASS) == InputType.TYPE_CLASS_TEXT
                    && (info.inputType & (InputType.TYPE_TEXT_FLAG_MULTI_LINE
                            | InputType.TYPE_TEXT_FLAG_IME_MULTI_LINE)) != 0;
            if (multiline) {
                // Keep literal newlines on the same ordered editor connection as text.
                connection.commitText("\n", 1);
                return;
            }
            if (action != EditorInfo.IME_ACTION_NONE && action != EditorInfo.IME_ACTION_UNSPECIFIED) {
                connection.performEditorAction(action);
                return;
            }
        }
        sendDownUpKeyEvents(code);
    }

    private void deleteText(InputConnection connection, boolean backwards) {
        if (!composition.isEmpty()) {
            // The host supplies the complete composition; shorten by one Unicode code point.
            int start = backwards ? 0 : composition.offsetByCodePoints(0, 1);
            int end = backwards ? composition.offsetByCodePoints(composition.length(), -1)
                    : composition.length();
            setComposition(connection, composition.substring(start, end));
            return;
        }
        CharSequence selected = connection.getSelectedText(0);
        if (selected != null && selected.length() > 0) {
            connection.commitText("", 1);
            return;
        }
        CharSequence adjacent = backwards ? connection.getTextBeforeCursor(2, 0)
                : connection.getTextAfterCursor(2, 0);
        if (adjacent != null && adjacent.length() > 0) {
            connection.deleteSurroundingTextInCodePoints(backwards ? 1 : 0, backwards ? 0 : 1);
        } else {
            sendDownUpKeyEvents(backwards ? KeyEvent.KEYCODE_DEL : KeyEvent.KEYCODE_FORWARD_DEL);
        }
    }

    private boolean moveCursor(InputConnection connection, int code) {
        ExtractedText extracted = connection.getExtractedText(new ExtractedTextRequest(), 0);
        if (extracted == null || extracted.text == null || extracted.partialStartOffset != -1
                || extracted.startOffset < 0 || extracted.selectionStart < 0 || extracted.selectionEnd < 0
                || extracted.selectionStart > extracted.text.length()
                || extracted.selectionEnd > extracted.text.length()) {
            return false;
        }
        CharSequence text = extracted.text;
        boolean backwards = code == KeyEvent.KEYCODE_DPAD_LEFT || code == KeyEvent.KEYCODE_MOVE_HOME;
        int position;
        if (code == KeyEvent.KEYCODE_MOVE_HOME) {
            if (extracted.startOffset != 0) {
                return false;
            }
            position = 0;
        } else if (code == KeyEvent.KEYCODE_MOVE_END) {
            position = text.length();
        } else if (extracted.selectionStart != extracted.selectionEnd) {
            position = backwards ? Math.min(extracted.selectionStart, extracted.selectionEnd)
                    : Math.max(extracted.selectionStart, extracted.selectionEnd);
        } else {
            position = extracted.selectionEnd;
            if (backwards && position > 0) {
                position = Character.offsetByCodePoints(text, position, -1);
            } else if (!backwards && position < text.length()) {
                position = Character.offsetByCodePoints(text, position, 1);
            } else if (backwards && extracted.startOffset > 0) {
                return false;
            }
        }
        // Also repair a selection supplied in the middle of a surrogate pair.
        if (position > 0 && position < text.length() && Character.isHighSurrogate(text.charAt(position - 1))
                && Character.isLowSurrogate(text.charAt(position))) {
            position += backwards ? -1 : 1;
        }
        int absolute = extracted.startOffset + position;
        return connection.setSelection(absolute, absolute);
    }

    private static int keyCode(String key) {
        switch (key) {
            case "enter": return KeyEvent.KEYCODE_ENTER;
            case "backspace": return KeyEvent.KEYCODE_DEL;
            case "delete": return KeyEvent.KEYCODE_FORWARD_DEL;
            case "tab": return KeyEvent.KEYCODE_TAB;
            case "escape": return KeyEvent.KEYCODE_ESCAPE;
            case "left": return KeyEvent.KEYCODE_DPAD_LEFT;
            case "right": return KeyEvent.KEYCODE_DPAD_RIGHT;
            case "up": return KeyEvent.KEYCODE_DPAD_UP;
            case "down": return KeyEvent.KEYCODE_DPAD_DOWN;
            case "home": return KeyEvent.KEYCODE_MOVE_HOME;
            case "end": return KeyEvent.KEYCODE_MOVE_END;
            default: return KeyEvent.KEYCODE_UNKNOWN;
        }
    }

    private static JSONObject frame(String op, Object... pairs) {
        JSONObject result = new JSONObject();
        try {
            result.put("op", op);
            for (int index = 0; index < pairs.length; index += 2) {
                result.put((String) pairs[index], pairs[index + 1]);
            }
        } catch (JSONException exception) {
            throw new IllegalArgumentException("Invalid protocol frame");
        }
        return result;
    }

    @Override
    public void onDestroy() {
        synchronized (socketLock) {
            stopped = true;
            if (server != null) {
                try {
                    Os.shutdown(server.getFileDescriptor(), OsConstants.SHUT_RDWR);
                } catch (ErrnoException ignored) {
                    // The listener may already have stopped.
                }
                closeQuietly(server);
            }
            if (client != null) {
                client.close();
                client = null;
            }
        }
        main.removeCallbacksAndMessages(null);
        super.onDestroy();
    }

    private static void closeQuietly(Closeable resource) {
        try {
            resource.close();
        } catch (IOException ignored) {
            // Do not log socket payloads or exception messages containing user text.
        }
    }

    private final class Client {
        private final LocalSocket socket;
        private final BufferedReader reader;
        private final BufferedWriter writer;
        private final ArrayBlockingQueue<String> outbound = new ArrayBlockingQueue<>(128);
        private final Semaphore pending = new Semaphore(128);
        private final Thread readThread;
        private final Thread writeThread;
        private volatile boolean closed;
        private boolean ready;

        Client(LocalSocket socket) throws IOException {
            this.socket = socket;
            reader = new BufferedReader(new InputStreamReader(socket.getInputStream(),
                    StandardCharsets.UTF_8.newDecoder()));
            writer = new BufferedWriter(new OutputStreamWriter(socket.getOutputStream(), StandardCharsets.UTF_8));
            readThread = new Thread(this::readCommands, "OmeIme-read");
            writeThread = new Thread(this::writeFrames, "OmeIme-write");
        }

        void start() {
            writeThread.start();
            readThread.start();
        }

        void send(JSONObject message) {
            if (!closed && !outbound.offer(message.toString())) {
                close();
            }
        }

        void readCommands() {
            try {
                StringBuilder line = new StringBuilder();
                int character;
                while (!closed && (character = reader.read()) != -1) {
                    if (character != '\n') {
                        if (line.length() >= MAX_FRAME_CHARS) {
                            Log.w(TAG, "Frame too long");
                            return;
                        }
                        line.append((char) character);
                        continue;
                    }
                    try {
                        JSONObject command = new JSONObject(line.toString());
                        long generation = inputGeneration;
                        pending.acquire();
                        main.post(() -> {
                            try {
                                if (!closed && client == this && generation == inputGeneration && !stopped) {
                                    apply(command);
                                }
                            } finally {
                                pending.release();
                            }
                        });
                    } catch (JSONException exception) {
                        Log.w(TAG, "Invalid JSON frame");
                    }
                    line.setLength(0);
                }
            } catch (IOException exception) {
                if (!closed) {
                    Log.d(TAG, "Peer disconnected");
                }
            } catch (InterruptedException exception) {
                Thread.currentThread().interrupt();
            } finally {
                close();
            }
        }

        void writeFrames() {
            try {
                while (!closed) {
                    writer.write(outbound.take());
                    writer.write('\n');
                    writer.flush();
                }
            } catch (IOException exception) {
                if (!closed) {
                    Log.d(TAG, "Peer write failed");
                }
            } catch (InterruptedException exception) {
                Thread.currentThread().interrupt();
            } finally {
                close();
            }
        }

        synchronized void close() {
            if (closed) {
                return;
            }
            closed = true;
            try {
                // close() alone does not wake a native read on a different thread.
                socket.shutdownInput();
            } catch (IOException ignored) {
                // The peer may have disconnected first.
            }
            try {
                socket.shutdownOutput();
            } catch (IOException ignored) {
                // Continue closing even if one half was already shut down.
            }
            closeQuietly(socket);
            readThread.interrupt();
            writeThread.interrupt();
        }
    }
}
