// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Placeholder by worker A. Worker B replaces this file with the S3 apps screen.
// Shell already provides <main>; render sections and divs. Shared parts: ../components.
import type { ReactElement } from 'react';
import type { ScreenProps } from '../actions';
import { ScreenHeader } from '../components';

export const AppsScreen: (props: ScreenProps) => ReactElement = () => <ScreenHeader title="앱" />;
