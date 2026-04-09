/* @ts-self-types="./gameboy_web.d.ts" */

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
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        GameBoyWebFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_gameboyweb_free(ptr, 0);
    }
    /**
     * Number of stereo sample *pairs* the APU has ready for reading.
     * @returns {number}
     */
    audio_samples_available() {
        const ret = wasm.gameboyweb_audio_samples_available(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Pointer to the audio staging buffer in WASM linear memory.
     * @returns {number}
     */
    audio_staging_ptr() {
        const ret = wasm.gameboyweb_audio_staging_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Copy all available audio samples into the staging buffer and return
     * its pointer so JS can create a `Float32Array` view.
     * Returns the number of **floats** written (pairs × 2).
     * @returns {number}
     */
    drain_audio_samples() {
        const ret = wasm.gameboyweb_drain_audio_samples(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Length of the frame buffer in bytes (160 × 144 × 4 = 92 160).
     * @returns {number}
     */
    frame_buffer_len() {
        const ret = wasm.gameboyweb_frame_buffer_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Pointer to the RGBA frame buffer inside WASM linear memory.
     * JS can build a `Uint8ClampedArray` view over this for `putImageData`.
     * @returns {number}
     */
    frame_buffer_ptr() {
        const ret = wasm.gameboyweb_frame_buffer_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Whether a ROM has been loaded.
     * @returns {boolean}
     */
    is_rom_loaded() {
        const ret = wasm.gameboyweb_is_rom_loaded(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Press a Game Boy button.  `btn` must be 0–7 (see `map_button`).
     * @param {number} btn
     */
    key_down(btn) {
        wasm.gameboyweb_key_down(this.__wbg_ptr, btn);
    }
    /**
     * Release a Game Boy button.
     * @param {number} btn
     */
    key_up(btn) {
        wasm.gameboyweb_key_up(this.__wbg_ptr, btn);
    }
    /**
     * Load a ROM from a `Uint8Array`.  Returns the cartridge title on
     * success or throws on invalid ROM data.
     * @param {Uint8Array} rom_data
     * @returns {string}
     */
    load_rom(rom_data) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passArray8ToWasm0(rom_data, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.gameboyweb_load_rom(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Create a new emulator instance.
     * `sample_rate` is the Web Audio API's sample rate (usually 44100 or 48000).
     * @param {number | null} [sample_rate]
     */
    constructor(sample_rate) {
        const ret = wasm.gameboyweb_new(isLikeNone(sample_rate) ? 0x100000001 : (sample_rate) >>> 0);
        this.__wbg_ptr = ret >>> 0;
        GameBoyWebFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Advance the emulator by exactly one frame (~70 224 CPU cycles).
     */
    run_frame() {
        wasm.gameboyweb_run_frame(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    screen_height() {
        const ret = wasm.gameboyweb_screen_height(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    screen_width() {
        const ret = wasm.gameboyweb_screen_width(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) GameBoyWeb.prototype[Symbol.dispose] = GameBoyWeb.prototype.free;

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_81fc77679af83bc6: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_debug_58754cc8dbfec7ec: function(arg0, arg1, arg2, arg3) {
            console.debug(arg0, arg1, arg2, arg3);
        },
        __wbg_error_38bec0a78dd8ded8: function(arg0) {
            console.error(arg0);
        },
        __wbg_error_f8d1622cb1d8c53c: function(arg0, arg1, arg2, arg3) {
            console.error(arg0, arg1, arg2, arg3);
        },
        __wbg_info_8e80eb6c0f1d9449: function(arg0, arg1, arg2, arg3) {
            console.info(arg0, arg1, arg2, arg3);
        },
        __wbg_log_dafe9ed5100e3a8c: function(arg0, arg1, arg2, arg3) {
            console.log(arg0, arg1, arg2, arg3);
        },
        __wbg_warn_b5013c1036317367: function(arg0, arg1, arg2, arg3) {
            console.warn(arg0, arg1, arg2, arg3);
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./gameboy_web_bg.js": import0,
    };
}

const GameBoyWebFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_gameboyweb_free(ptr >>> 0, 1));

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('gameboy_web_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
