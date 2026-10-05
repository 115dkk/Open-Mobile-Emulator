// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
package org.ome.videoprobe;

import android.app.Activity;
import android.content.res.AssetFileDescriptor;
import android.graphics.SurfaceTexture;
import android.media.MediaPlayer;
import android.opengl.GLES11Ext;
import android.opengl.GLES20;
import android.opengl.GLSurfaceView;
import android.os.Bundle;
import android.os.SystemClock;
import android.util.Log;
import android.view.Surface;
import android.widget.VideoView;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.FloatBuffer;

import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.opengles.GL10;

/**
 * Plays assets/probe.mp4 the way game engines do, without native code, and logs objective facts.
 *
 * Start with `am start -n org.ome.videoprobe/.MainActivity --es mode gl|view`.
 * Mode `gl` (default) decodes into a SurfaceTexture sampled as an external OES texture, the path
 * Unity's VideoPlayer takes. Mode `view` lets VideoView hand the decoder a window surface.
 * Every line is tagged OMEVideo: prepared, first frame, per-second frame count, the centre pixel
 * and a green control corner (gl mode), completion and errors.
 */
public final class MainActivity extends Activity {
    private static final String TAG = "OMEVideo";
    private final long startedAt = SystemClock.elapsedRealtime();
    private GLSurfaceView glView;
    private MediaPlayer player;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        String mode = getIntent().getStringExtra("mode");
        log("start mode=" + (mode == null ? "gl" : mode));
        if ("view".equals(mode)) {
            startVideoView();
        } else {
            startGl();
        }
    }

    private void startVideoView() {
        VideoView view = new VideoView(this);
        setContentView(view);
        view.setVideoPath("android.resource://" + getPackageName() + "/raw/probe");
        view.setOnPreparedListener(mp -> {
            log("prepared width=" + mp.getVideoWidth() + " height=" + mp.getVideoHeight()
                    + " durationMs=" + mp.getDuration());
            mp.setOnInfoListener((p, what, extra) -> {
                if (what == MediaPlayer.MEDIA_INFO_VIDEO_RENDERING_START) {
                    log("first-frame");
                } else {
                    log("info what=" + what + " extra=" + extra);
                }
                return false;
            });
        });
        view.setOnCompletionListener(mp -> log("completed"));
        view.setOnErrorListener((mp, what, extra) -> {
            log("error what=" + what + " extra=" + extra);
            return true;
        });
        view.start();
    }

    private void startGl() {
        glView = new GLSurfaceView(this);
        glView.setEGLContextClientVersion(2);
        glView.setRenderer(new ProbeRenderer());
        glView.setRenderMode(GLSurfaceView.RENDERMODE_WHEN_DIRTY);
        setContentView(glView);
    }

    @Override
    protected void onDestroy() {
        if (player != null) {
            player.release();
            player = null;
        }
        super.onDestroy();
    }

    private void log(String message) {
        Log.i(TAG, "t=" + (SystemClock.elapsedRealtime() - startedAt) + "ms " + message);
    }

    private final class ProbeRenderer implements GLSurfaceView.Renderer,
            SurfaceTexture.OnFrameAvailableListener {
        private static final String VERTEX =
                "attribute vec4 aPos;\n"
                + "attribute vec2 aTex;\n"
                + "uniform mat4 uTex;\n"
                + "varying vec2 vTex;\n"
                + "void main() {\n"
                + "  gl_Position = aPos;\n"
                + "  vTex = (uTex * vec4(aTex, 0.0, 1.0)).xy;\n"
                + "}\n";
        private static final String FRAGMENT =
                "#extension GL_OES_EGL_image_external : require\n"
                + "precision mediump float;\n"
                + "uniform samplerExternalOES uSampler;\n"
                + "varying vec2 vTex;\n"
                + "void main() { gl_FragColor = texture2D(uSampler, vTex); }\n";
        private final float[] texMatrix = new float[16];
        private final FloatBuffer quad = ByteBuffer.allocateDirect(4 * 4 * 4)
                .order(ByteOrder.nativeOrder()).asFloatBuffer()
                .put(new float[] {
                    -1f, -1f, 0f, 0f,
                     1f, -1f, 1f, 0f,
                    -1f,  1f, 0f, 1f,
                     1f,  1f, 1f, 1f,
                });
        private final ByteBuffer pixel = ByteBuffer.allocateDirect(4).order(ByteOrder.nativeOrder());
        private SurfaceTexture surfaceTexture;
        private int program;
        private int texture;
        private int width;
        private int height;
        private int framesThisSecond;
        private long totalFrames;
        private long secondStartedAt;
        private boolean firstFrameLogged;

        @Override
        public void onSurfaceCreated(GL10 unused, EGLConfig config) {
            log("gl renderer=" + GLES20.glGetString(GLES20.GL_RENDERER)
                    + " version=" + GLES20.glGetString(GLES20.GL_VERSION));
            int[] names = new int[1];
            GLES20.glGenTextures(1, names, 0);
            texture = names[0];
            GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, texture);
            GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES,
                    GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR);
            GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES,
                    GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR);
            program = link(compile(GLES20.GL_VERTEX_SHADER, VERTEX),
                    compile(GLES20.GL_FRAGMENT_SHADER, FRAGMENT));
            surfaceTexture = new SurfaceTexture(texture);
            surfaceTexture.setOnFrameAvailableListener(this);
            startPlayer(new Surface(surfaceTexture));
        }

        private void startPlayer(Surface surface) {
            MediaPlayer media = new MediaPlayer();
            try (AssetFileDescriptor file = getResources().openRawResourceFd(R.raw.probe)) {
                media.setDataSource(file.getFileDescriptor(), file.getStartOffset(), file.getLength());
            } catch (IOException exception) {
                log("error open " + exception.getClass().getSimpleName());
                return;
            }
            media.setSurface(surface);
            media.setOnPreparedListener(mp -> {
                log("prepared width=" + mp.getVideoWidth() + " height=" + mp.getVideoHeight()
                        + " durationMs=" + mp.getDuration());
                mp.start();
            });
            media.setOnCompletionListener(mp -> log("completed frames=" + totalFrames));
            media.setOnErrorListener((mp, what, extra) -> {
                log("error what=" + what + " extra=" + extra);
                return true;
            });
            media.setOnInfoListener((mp, what, extra) -> {
                log("info what=" + what + " extra=" + extra);
                return false;
            });
            player = media;
            media.prepareAsync();
        }

        @Override
        public void onFrameAvailable(SurfaceTexture texture) {
            glView.requestRender();
        }

        @Override
        public void onSurfaceChanged(GL10 unused, int newWidth, int newHeight) {
            width = newWidth;
            height = newHeight;
            GLES20.glViewport(0, 0, width, height);
        }

        @Override
        public void onDrawFrame(GL10 unused) {
            surfaceTexture.updateTexImage();
            surfaceTexture.getTransformMatrix(texMatrix);
            GLES20.glClearColor(1f, 0f, 1f, 1f);
            GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT);
            GLES20.glUseProgram(program);
            int position = GLES20.glGetAttribLocation(program, "aPos");
            int coordinate = GLES20.glGetAttribLocation(program, "aTex");
            quad.position(0);
            GLES20.glVertexAttribPointer(position, 2, GLES20.GL_FLOAT, false, 16, quad);
            GLES20.glEnableVertexAttribArray(position);
            quad.position(2);
            GLES20.glVertexAttribPointer(coordinate, 2, GLES20.GL_FLOAT, false, 16, quad);
            GLES20.glEnableVertexAttribArray(coordinate);
            GLES20.glUniformMatrix4fv(GLES20.glGetUniformLocation(program, "uTex"), 1, false, texMatrix, 0);
            GLES20.glActiveTexture(GLES20.GL_TEXTURE0);
            GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, texture);
            GLES20.glUniform1i(GLES20.glGetUniformLocation(program, "uSampler"), 0);
            GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4);
            // A corner the video does not touch: cleared to green every frame. If it stops reading
            // green, the whole GL context stopped rendering (not just the video texture).
            GLES20.glEnable(GLES20.GL_SCISSOR_TEST);
            GLES20.glScissor(0, 0, 8, 8);
            GLES20.glClearColor(0f, 1f, 0f, 1f);
            GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT);
            GLES20.glDisable(GLES20.GL_SCISSOR_TEST);

            totalFrames++;
            framesThisSecond++;
            long now = SystemClock.elapsedRealtime();
            if (!firstFrameLogged) {
                firstFrameLogged = true;
                secondStartedAt = now;
                log("first-frame " + centre());
            } else if (now - secondStartedAt >= 1000) {
                log("frames=" + framesThisSecond + " total=" + totalFrames + " " + centre()
                        + " glError=" + GLES20.glGetError());
                framesThisSecond = 0;
                secondStartedAt = now;
            }
        }

        /**
         * Reads the centre pixel (magenta: the quad was not drawn, black: empty video texture) and
         * the green corner (anything else: the context no longer renders at all).
         */
        private String centre() {
            return "centre=" + read(width / 2, height / 2) + " corner=" + read(2, 2);
        }

        private String read(int x, int y) {
            pixel.clear();
            GLES20.glReadPixels(x, y, 1, 1, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, pixel);
            return (pixel.get(0) & 0xff) + "," + (pixel.get(1) & 0xff) + "," + (pixel.get(2) & 0xff);
        }

        private int compile(int type, String source) {
            int shader = GLES20.glCreateShader(type);
            GLES20.glShaderSource(shader, source);
            GLES20.glCompileShader(shader);
            int[] status = new int[1];
            GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, status, 0);
            if (status[0] == 0) {
                log("error shader " + GLES20.glGetShaderInfoLog(shader));
            }
            return shader;
        }

        private int link(int vertex, int fragment) {
            int linked = GLES20.glCreateProgram();
            GLES20.glAttachShader(linked, vertex);
            GLES20.glAttachShader(linked, fragment);
            GLES20.glLinkProgram(linked);
            int[] status = new int[1];
            GLES20.glGetProgramiv(linked, GLES20.GL_LINK_STATUS, status, 0);
            if (status[0] == 0) {
                log("error link " + GLES20.glGetProgramInfoLog(linked));
            }
            return linked;
        }
    }
}
