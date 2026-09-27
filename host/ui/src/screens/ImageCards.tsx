// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Operating-system image cards (S1.4), shared by the wizard and the settings dialog 새 운영체제 설치.
// Rust sorts the profiles and marks the recommended one; the cards only draw them.
import type { GuestImageSummary } from '../contracts';
import { Chip } from '../components';
import { formatBytes } from '../format';
import { IMAGE_STATUS_LABEL, IMAGE_STATUS_TONE } from '../presentation';

export function ImageCards({ images, selectedId, disabled, onSelect }: {
  readonly images: readonly GuestImageSummary[];
  readonly selectedId: string | null;
  readonly disabled: boolean;
  readonly onSelect: (id: string) => void;
}) {
  return (
    <div className="ome-image-cards" role="radiogroup" aria-label="운영체제 이미지">
      {images.map((image) => {
        const checked = image.id === selectedId;
        return (
          <button
            key={image.id}
            type="button"
            role="radio"
            aria-checked={checked}
            className="ome-image-card"
            disabled={disabled}
            onClick={() => { if (!checked) onSelect(image.id); }}
          >
            <span className="ome-image-card-head">
              <span className="ome-image-card-name">{image.displayName}</span>
              <Chip tone={IMAGE_STATUS_TONE[image.status]}>{IMAGE_STATUS_LABEL[image.status]}</Chip>
            </span>
            <span className="ome-image-card-meta">
              <span>API {image.apiLevel}</span>
              {image.sizeBytes !== null && <span>{formatBytes(image.sizeBytes)}</span>}
            </span>
            <span className="ome-image-card-meta">검증된 게임 {image.verifiedGames}개</span>
          </button>
        );
      })}
    </div>
  );
}
