/* tslint:disable */
/* eslint-disable */

/**
 * Wrapper around the Game Boy emulator for the web frontend.
 *
 * Exposes the emulator through wasm-bindgen so JavaScript can:
 *   - Load ROMs (as byte arrays)
 *   - Run one frame of emulation
 *   - Read the RGBA frame-buffer directly from WASM linear memory
 *   - Read audio samples directly from WASM linear memory
 *   - Send key-down/key-up events
 */
export class GameBoyWeb {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Number of stereo sample *pairs* the APU has ready for reading.
     */
    audio_samples_available(): number;
    /**
     * Pointer to the audio staging buffer in WASM linear memory.
     */
    audio_staging_ptr(): number;
    /**
     * Copy all available audio samples into the staging buffer and return
     * its pointer so JS can create a `Float32Array` view.
     * Returns the number of **floats** written (pairs × 2).
     */
    drain_audio_samples(): number;
    /**
     * Length of the frame buffer in bytes (160 × 144 × 4 = 92 160).
     */
    frame_buffer_len(): number;
    /**
     * Pointer to the RGBA frame buffer inside WASM linear memory.
     * JS can build a `Uint8ClampedArray` view over this for `putImageData`.
     */
    frame_buffer_ptr(): number;
    /**
     * Whether a ROM has been loaded.
     */
    is_rom_loaded(): boolean;
    /**
     * Press a Game Boy button.  `btn` must be 0–7 (see `map_button`).
     */
    key_down(btn: number): void;
    /**
     * Release a Game Boy button.
     */
    key_up(btn: number): void;
    /**
     * Load a ROM from a `Uint8Array`.  Returns the cartridge title on
     * success or throws on invalid ROM data.
     */
    load_rom(rom_data: Uint8Array): string;
    /**
     * Create a new emulator instance.
     * `sample_rate` is the Web Audio API's sample rate (usually 44100 or 48000).
     */
    constructor(sample_rate?: number | null);
    /**
     * Advance the emulator by exactly one frame (~70 224 CPU cycles).
     */
    run_frame(): void;
    screen_height(): number;
    screen_width(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_gameboyweb_free: (a: number, b: number) => void;
    readonly gameboyweb_audio_samples_available: (a: number) => number;
    readonly gameboyweb_audio_staging_ptr: (a: number) => number;
    readonly gameboyweb_drain_audio_samples: (a: number) => number;
    readonly gameboyweb_frame_buffer_len: (a: number) => number;
    readonly gameboyweb_frame_buffer_ptr: (a: number) => number;
    readonly gameboyweb_is_rom_loaded: (a: number) => number;
    readonly gameboyweb_key_down: (a: number, b: number) => void;
    readonly gameboyweb_key_up: (a: number, b: number) => void;
    readonly gameboyweb_load_rom: (a: number, b: number, c: number) => [number, number, number, number];
    readonly gameboyweb_new: (a: number) => number;
    readonly gameboyweb_run_frame: (a: number) => void;
    readonly gameboyweb_screen_height: (a: number) => number;
    readonly gameboyweb_screen_width: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
