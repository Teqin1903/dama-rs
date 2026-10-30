let wasm_bindgen = (function(exports) {
    let script_src;
    if (typeof document !== 'undefined' && document.currentScript !== null) {
        script_src = new URL(document.currentScript.src, location.href).toString();
    }

    /**
     * One game and its agents (`engine::Session`).
     */
    class Session {
        __destroy_into_raw() {
            const ptr = this.__wbg_ptr;
            this.__wbg_ptr = 0;
            SessionFinalization.unregister(this);
            return ptr;
        }
        free() {
            const ptr = this.__destroy_into_raw();
            wasm.__wbg_session_free(ptr, 0);
        }
        /**
         * @param {string} spec
         * @param {string} options
         * @returns {string}
         */
        analyze(spec, options) {
            let deferred3_0;
            let deferred3_1;
            try {
                let ptr0, len0, ptr1, len1;
                try {
                    ptr0 = passStringToWasm0(spec, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len0 = WASM_VECTOR_LEN;
                    ptr1 = passStringToWasm0(options, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len1 = WASM_VECTOR_LEN;
                } catch (e) {
                    if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                    if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                    throw e;
                }
                const ret = wasm.session_analyze(this.__wbg_ptr, ptr0, len0, ptr1, len1);
                deferred3_0 = ret[0];
                deferred3_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
            }
        }
        /**
         * @param {string} weights
         * @param {string} options
         * @returns {string}
         */
        eval(weights, options) {
            let deferred3_0;
            let deferred3_1;
            try {
                let ptr0, len0, ptr1, len1;
                try {
                    ptr0 = passStringToWasm0(weights, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len0 = WASM_VECTOR_LEN;
                    ptr1 = passStringToWasm0(options, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len1 = WASM_VECTOR_LEN;
                } catch (e) {
                    if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                    if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                    throw e;
                }
                const ret = wasm.session_eval(this.__wbg_ptr, ptr0, len0, ptr1, len1);
                deferred3_0 = ret[0];
                deferred3_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
            }
        }
        /**
         * @returns {string}
         */
        legal() {
            let deferred1_0;
            let deferred1_1;
            try {
                const ret = wasm.session_legal(this.__wbg_ptr);
                deferred1_0 = ret[0];
                deferred1_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
            }
        }
        /**
         * The start position, White to move, no agents.
         */
        constructor() {
            const ret = wasm.session_new();
            this.__wbg_ptr = ret;
            SessionFinalization.register(this, this.__wbg_ptr, this);
            return this;
        }
        /**
         * @param {string} mv
         * @returns {string}
         */
        play(mv) {
            let deferred2_0;
            let deferred2_1;
            try {
                const ptr0 = passStringToWasm0(mv, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                const len0 = WASM_VECTOR_LEN;
                const ret = wasm.session_play(this.__wbg_ptr, ptr0, len0);
                deferred2_0 = ret[0];
                deferred2_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
            }
        }
        /**
         * @param {string} options
         * @returns {string}
         */
        position(options) {
            let deferred2_0;
            let deferred2_1;
            try {
                const ptr0 = passStringToWasm0(options, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                const len0 = WASM_VECTOR_LEN;
                const ret = wasm.session_position(this.__wbg_ptr, ptr0, len0);
                deferred2_0 = ret[0];
                deferred2_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
            }
        }
        /**
         * @param {string} board
         * @param {string} turn
         * @returns {string}
         */
        reset(board, turn) {
            let deferred3_0;
            let deferred3_1;
            try {
                let ptr0, len0, ptr1, len1;
                try {
                    ptr0 = passStringToWasm0(board, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len0 = WASM_VECTOR_LEN;
                    ptr1 = passStringToWasm0(turn, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len1 = WASM_VECTOR_LEN;
                } catch (e) {
                    if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                    if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                    throw e;
                }
                const ret = wasm.session_reset(this.__wbg_ptr, ptr0, len0, ptr1, len1);
                deferred3_0 = ret[0];
                deferred3_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
            }
        }
        /**
         * @param {string} side
         * @param {string} spec
         * @param {number} seed
         * @returns {string}
         */
        set_agent(side, spec, seed) {
            let deferred4_0;
            let deferred4_1;
            try {
                const val3 = +seed;
                let ptr0, len0, ptr1, len1;
                try {
                    ptr0 = passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len0 = WASM_VECTOR_LEN;
                    ptr1 = passStringToWasm0(spec, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len1 = WASM_VECTOR_LEN;
                } catch (e) {
                    if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                    if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                    throw e;
                }
                const ret = wasm.session_set_agent(this.__wbg_ptr, ptr0, len0, ptr1, len1, val3);
                deferred4_0 = ret[0];
                deferred4_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
            }
        }
        /**
         * @returns {string}
         */
        state() {
            let deferred1_0;
            let deferred1_1;
            try {
                const ret = wasm.session_state(this.__wbg_ptr);
                deferred1_0 = ret[0];
                deferred1_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
            }
        }
        /**
         * @param {string} side
         * @param {string} options
         * @returns {string}
         */
        think(side, options) {
            let deferred3_0;
            let deferred3_1;
            try {
                let ptr0, len0, ptr1, len1;
                try {
                    ptr0 = passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len0 = WASM_VECTOR_LEN;
                    ptr1 = passStringToWasm0(options, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                    len1 = WASM_VECTOR_LEN;
                } catch (e) {
                    if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                    if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                    throw e;
                }
                const ret = wasm.session_think(this.__wbg_ptr, ptr0, len0, ptr1, len1);
                deferred3_0 = ret[0];
                deferred3_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
            }
        }
        /**
         * @returns {string}
         */
        undo() {
            let deferred1_0;
            let deferred1_1;
            try {
                const ret = wasm.session_undo(this.__wbg_ptr);
                deferred1_0 = ret[0];
                deferred1_1 = ret[1];
                return getStringFromWasm0(ret[0], ret[1]);
            } finally {
                wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
            }
        }
    }
    if (Symbol.dispose) Session.prototype[Symbol.dispose] = Session.prototype.free;
    exports.Session = Session;

    /**
     * `{"engine": "<git describe at build time>", "specs": [...]}`.
     * @returns {string}
     */
    function version() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.version();
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    exports.version = version;
    function __wbg_get_imports() {
        const import0 = {
            __proto__: null,
            __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
                throw new Error(getStringFromWasm0(arg0, arg1));
            },
            __wbg_error_757e9472f8410341: function(arg0, arg1) {
                let deferred0_0;
                let deferred0_1;
                try {
                    deferred0_0 = arg0;
                    deferred0_1 = arg1;
                    console.error(getStringFromWasm0(arg0, arg1));
                } finally {
                    wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
                }
            },
            __wbg_new_bb74001155cb8ca8: function() {
                const ret = new Error();
                return ret;
            },
            __wbg_now_3b57bc9f03d35bf8: function() {
                const ret = Date.now();
                return ret;
            },
            __wbg_stack_575c4e0918581b9c: function(arg0, arg1) {
                const ret = arg1.stack;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
                getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
            },
            __wbindgen_init_externref_table: function() {
                const table = wasm.__wbindgen_externrefs;
                table.set(0, undefined);
                table.set(1024, undefined);
                table.set(1025, null);
                table.set(1026, true);
                table.set(1027, false);
            },
        };
        return {
            __proto__: null,
            "./dama_web_bg.js": import0,
        };
    }

    const SessionFinalization = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(ptr => wasm.__wbg_session_free(ptr, 1));

    let cachedDataViewMemory0 = null;
    function getDataViewMemory0() {
        if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
            cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
        }
        return cachedDataViewMemory0;
    }

    function getStringFromWasm0(ptr, len) {
        ptr = ptr >>> 0;
        return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
    }

    let cachedUint8ArrayMemory0 = null;
    function getUint8ArrayMemory0() {
        if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
            cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
        }
        return cachedUint8ArrayMemory0;
    }

    function passStringToWasm0(arg, malloc, realloc) {
        if (realloc === undefined) {
            const buf = cachedTextEncoder.encode(arg);
            const ptr = malloc(buf.length, 1) >>> 0;
            getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
            WASM_VECTOR_LEN = buf.length;
            return ptr;
        }

        let len = arg.length;
        let ptr = malloc(len, 1) >>> 0;

        const mem = getUint8ArrayMemory0();

        let offset = 0;

        for (; offset < len; offset++) {
            const code = arg.charCodeAt(offset);
            if (code > 0x7F) break;
            mem[ptr + offset] = code;
        }
        if (offset !== len) {
            if (offset !== 0) {
                arg = arg.slice(offset);
            }
            ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
            const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
            const ret = cachedTextEncoder.encodeInto(arg, view);

            offset += ret.written;
            ptr = realloc(ptr, len, offset, 1) >>> 0;
        }

        WASM_VECTOR_LEN = offset;
        return ptr;
    }

    let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
    cachedTextDecoder.decode();
    function decodeText(bytes) {
        return cachedTextDecoder.decode(bytes);
    }

    const cachedTextEncoder = new TextEncoder();

    if (!('encodeInto' in cachedTextEncoder)) {
        cachedTextEncoder.encodeInto = function (arg, view) {
            const buf = cachedTextEncoder.encode(arg);
            view.set(buf);
            return {
                read: arg.length,
                written: buf.length
            };
        };
    }

    let WASM_VECTOR_LEN = 0;

    let wasmModule, wasmInstance, wasm;
    function __wbg_finalize_init(instance, module) {
        wasmInstance = instance;
        wasm = instance.exports;
        wasmModule = module;
        cachedDataViewMemory0 = null;
        cachedUint8ArrayMemory0 = null;
        wasm.__wbindgen_start();
        return wasm;
    }

    async function __wbg_load(module, imports) {
        if (typeof Response === 'function' && module instanceof Response) {
            if (!module.ok) {
                throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
            }

            if (typeof WebAssembly.instantiateStreaming === 'function') {
                try {
                    return await WebAssembly.instantiateStreaming(module, imports);
                } catch (e) {
                    const validResponse = expectedResponseType(module.type);

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

    let __wbg_init_promise;
    async function __wbg_init(module_or_path) {
        if (wasm !== undefined) return wasm;
        if (__wbg_init_promise === undefined) {
            __wbg_init_promise = __wbg_init_once(module_or_path);
            __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
        }
        return __wbg_init_promise;
    }

    async function __wbg_init_once(module_or_path) {

        if (module_or_path !== undefined) {
            if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
                ({module_or_path} = module_or_path)
            } else {
                console.warn('using deprecated parameters for the initialization function; pass a single object instead')
            }
        }

        if (module_or_path === undefined && script_src !== undefined) {
            module_or_path = script_src.replace(/\.js$/, "_bg.wasm");
        }
        const imports = __wbg_get_imports();

        if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
            module_or_path = fetch(module_or_path);
        }

        const { instance, module } = await __wbg_load(await module_or_path, imports);

        return __wbg_finalize_init(instance, module);
    }

    return Object.assign(__wbg_init, { initSync }, exports);
})({ __proto__: null });
