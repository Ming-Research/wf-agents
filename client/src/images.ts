// The images the owner sends with words, from a box of the page: which images the box takes and what it refuses, said
// plainly; what a paste adds; how an image waits in the box; and how they go to the interface with the words, as one
// message. The interface takes the same images and refuses the rest. They go to the agents in their messages, the
// clean way, with no file for an agent to find or be let read.

import type { ImageSent } from './api';
import { t } from './messages';

/// The kinds of image the owner may send, by their media types.
export const IMAGE_KINDS: readonly string[] = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

/// The most bytes one image may hold: 5 MB.
export const IMAGE_MOST_BYTES = 5 * 1024 * 1024;

/// The most images one message may carry.
export const IMAGES_MOST = 10;

/// An image the owner added to a box, waiting to go with the words: its name, its kind, its size, and its bytes as a
/// data URL, which both shows it and is sent.
export type PendingImage = { key: string; name: string; media_type: string; size: number; data_url: string };

/// A file as the picker and the clipboard give it.
export type FileLike = { name: string; type: string; size: number };

/// A size in megabytes, to one decimal.
export const megabytes = (bytes: number): string => (bytes / (1024 * 1024)).toFixed(1);

/// Of the files added to a box with `waiting` images in it already, those it takes, and what it refuses, said plainly
/// for each: a file that is no PNG, JPEG, GIF or WebP image, an image over 5 MB, and one past the 10 a message carries.
export function admit<F extends FileLike>(waiting: number, files: readonly F[]): { take: F[]; refused: string[] } {
  const take: F[] = [];
  const refused: string[] = [];
  for (const f of files) {
    const name = f.name !== '' ? f.name : t('image_pasted');
    if (!IMAGE_KINDS.includes(f.type)) refused.push(t('image_refused_kind', { name }));
    else if (f.size > IMAGE_MOST_BYTES) refused.push(t('image_refused_size', { name, size: megabytes(f.size) }));
    else if (waiting + take.length >= IMAGES_MOST) refused.push(t('image_refused_most', { name, most: IMAGES_MOST }));
    else take.push(f);
  }
  return { take, refused };
}

/// What a paste into a box adds as images: the files it carries when it carries no text; none when it carries text,
/// which pastes as text, as it always did.
export function pastedFiles<F>(types: readonly string[], files: readonly F[]): F[] {
  return types.includes('text/plain') ? [] : [...files];
}

/// Whether words can be sent: they say something, or images go with them.
export const saysSomething = (text: string, images: readonly PendingImage[]): boolean =>
  text.trim() !== '' || images.length > 0;

/// The images as the interface takes them, in the order added: each its media type and its bytes as base64, out of its
/// data URL.
export function imagesSent(images: readonly PendingImage[]): ImageSent[] {
  return images.map((i) => ({ media_type: i.media_type, data: i.data_url.slice(i.data_url.indexOf(',') + 1) }));
}

let added = 0;

/// Reads a file the box took into an image waiting in it.
export function readImage(f: File): Promise<PendingImage> {
  return new Promise((done, failed) => {
    const reader = new FileReader();
    reader.onload = () => {
      added += 1;
      done({ key: `image-${added}`, name: f.name, media_type: f.type, size: f.size, data_url: String(reader.result) });
    };
    reader.onerror = () => failed(reader.error ?? new Error(t('image_unreadable', { name: f.name })));
    reader.readAsDataURL(f);
  });
}

/// Where the page reads an image the owner sent, by its id: as it was sent.
export const imageHref = (id: number): string => `/api/v1/attachments/${id}`;
