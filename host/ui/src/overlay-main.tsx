// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Entry of the transparent `overlay` window (overlay.html). Same bridge and controller as the main window.
import { createElement, StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { controllerBridge } from './bridge';
import { OverlayWindow } from './overlay/OverlayWindow';
import './styles/tokens.css';
import './styles/app.css';
import './styles/overlay.css';

const root = document.getElementById('root');
if (!root) throw new Error('Overlay root is missing.');
createRoot(root).render(createElement(StrictMode, null, createElement(OverlayWindow, { bridge: controllerBridge })));
