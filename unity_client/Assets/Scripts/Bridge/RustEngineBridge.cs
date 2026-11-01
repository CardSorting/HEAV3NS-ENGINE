using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using UnityEngine;

namespace Heav3ns.UnityBridge
{
    /// <summary>
    /// Native P/Invoke ([DllImport]) bindings and fallback bridge to the deterministic HEAV3NS-ENGINE Rust core.
    /// Supports both direct native dynamic library calls (libheav3ns_engine.dylib / .dll / .so in Assets/Plugins)
    /// and JSON command/event exchange for the VisualEventQueue.
    /// </summary>
    public static class RustEngineBridge
    {
#if UNITY_IOS && !UNITY_EDITOR
        public const string DllName = "__Internal";
#else
        public const string DllName = "heav3ns_engine";
#endif

        // ====================================================================
        // Native C-ABI [DllImport] Stubs for HEAV3NS-ENGINE Dynamic Library
        // ====================================================================

        /// <summary>
        /// Creates a new seeded Wonderland match instance in the native Rust engine.
        /// Returns an opaque handle pointer (IntPtr.Zero on error).
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_create")]
        public static extern IntPtr NativeCreateMatch(ulong seed, uint starterSeat);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_create_with_deck")]
        public static extern IntPtr NativeCreateMatchWithDeck(ulong seed, uint starterSeat, [MarshalAs(UnmanagedType.LPUTF8Str)] string deckJson);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_create_with_scenario")]
        public static extern IntPtr NativeCreateMatchWithScenario(ulong seed, uint starterSeat, [MarshalAs(UnmanagedType.LPUTF8Str)] string deckJson, uint stage);

        /// <summary>
        /// Destroys a previously created native match handle and frees its memory.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_destroy")]
        public static extern void NativeDestroyMatch(IntPtr engineHandle);

        /// <summary>
        /// Submits a serialized GameCommand JSON payload to the native Rust engine.
        /// Returns a pointer to a UTF-8 JSON response string (freed via NativeFreeString).
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_submit_command_json")]
        public static extern IntPtr NativeSubmitCommandJson(
            IntPtr engineHandle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string commandJson);

        /// <summary>
        /// Drains pending VisualEvent items emitted by the Rust engine as a UTF-8 JSON array string.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_drain_visual_events_json")]
        public static extern IntPtr NativeDrainVisualEventsJson(IntPtr engineHandle);

        /// <summary>
        /// Returns the current redacted VisualBoardState snapshot as a UTF-8 JSON string.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_get_board_state_json")]
        public static extern IntPtr NativeGetBoardStateJson(IntPtr engineHandle);

        /// <summary>
        /// Frees a C string allocated and returned by the Rust dynamic library.
        /// </summary>
        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "heav3ns_engine_free_string")]
        public static extern void NativeFreeString(IntPtr strPtr);

        /// <summary>
        /// Helper to safely marshal a Rust-allocated UTF-8 string and free the native buffer.
        /// </summary>
        public static string ConsumeNativeString(IntPtr ptr)
        {
            if (ptr == IntPtr.Zero)
            {
                return string.Empty;
            }

            try
            {
                return Marshal.PtrToStringUTF8(ptr) ?? string.Empty;
            }
            finally
            {
                try
                {
                    NativeFreeString(ptr);
                }
                catch (DllNotFoundException)
                {
                    // Stub safety when native library is not yet loaded
                }
                catch (EntryPointNotFoundException)
                {
                    // Stub safety when native entry point is not bound
                }
            }
        }

        /// <summary>
        /// Submits a serialized JSON GameCommand to a native match handle and returns the UTF-8 response string.
        /// </summary>
        public static string SubmitCommandJson(IntPtr engineHandle, string commandJson)
        {
            if (engineHandle == IntPtr.Zero || string.IsNullOrWhiteSpace(commandJson))
            {
                return "{\"status\":\"error\",\"reason\":\"invalid_handle_or_command\"}";
            }
            IntPtr respPtr = NativeSubmitCommandJson(engineHandle, commandJson);
            return ConsumeNativeString(respPtr);
        }

        /// <summary>
        /// Checks whether the native Rust dynamic library is loaded and reachable in Assets/Plugins.
        /// </summary>
        public static bool IsNativeLibraryAvailable()
        {
            try
            {
                IntPtr handle = NativeCreateMatch(42UL, 0U);
                if (handle != IntPtr.Zero)
                {
                    NativeDestroyMatch(handle);
                }
                return true;
            }
            catch (DllNotFoundException)
            {
                return false;
            }
            catch (EntryPointNotFoundException)
            {
                return false;
            }
        }
    }
}

