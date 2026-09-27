// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { createElement, StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Gallery } from './qa-gallery';
import './styles/tokens.css';
import './styles/app.css';
import './styles/qa-gallery.css';

const root = document.getElementById('root');
if (!root) throw new Error('Gallery root is missing.');
createRoot(root).render(createElement(StrictMode, null, createElement(Gallery)));
