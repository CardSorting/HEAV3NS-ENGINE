using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Text;
using UnityEngine;

namespace Heav3ns.UnityBridge
{
    /// <summary>
    /// Serializable representation of a VisualEvent drained from the Rust core.
    /// Mirrors heav3ns_engine / client_macroquad::bridge::VisualEvent.
    /// </summary>
    [Serializable]
    public struct VisualEventRecord
    {
        public string eventType;
        public int seat;
        public int turnNumber;
        public int cardUid;
        public string cardSpecId;
        public string title;
        public int cost;
        public string cardType;
        public int slotIndex;
        public int targetUid;
        public int amount;
        public int remaining;
        public bool isShieldBreak;
        public string triggerText;
        public string reason;
        public string text;
        public string subtext;
        public bool isBlocking;
        public float duration;
    }

    /// <summary>
    /// Wrapper for JSON array deserialization via Unity's JsonUtility.
    /// </summary>
    [Serializable]
    public class VisualEventBatchPayload
    {
        public List<VisualEventRecord> events = new List<VisualEventRecord>();
    }

    /// <summary>
    /// MonoBehaviour that drains serialized JSON/struct VisualEvents from the Rust core
    /// and sequences blocking vs. concurrent presentation animations on the Unity 2.5D card stage.
    /// Note: Must live in VisualEventQueue.cs so Unity serializes the MonoBehaviour on scene GameObjects!
    /// </summary>
    public class VisualEventQueue : MonoBehaviour
    {
        [Header("Bridge Configuration")]
        [SerializeField] private ulong initialSeed = 42UL;
        [SerializeField] private uint starterSeat = 0U;
        [SerializeField] private bool pollNativeEngineOnUpdate = true;
        [SerializeField] private float playbackSpeedMultiplier = 1.0f;

        private readonly ConcurrentQueue<VisualEventRecord> _pendingQueue = new ConcurrentQueue<VisualEventRecord>();
        private readonly List<VisualEventRecord> _completedEvents = new List<VisualEventRecord>();

        private IntPtr _nativeHandle = IntPtr.Zero;
        private bool _nativeAvailable;
        private string _customDeckJson;
        private uint _customStage;
        private VisualEventRecord? _activeBlockingEvent;
        private float _activeBlockingRemainingTime;

        public event Action<VisualEventRecord> OnVisualEventDispatched;
        public event Action<VisualEventRecord> OnBlockingEventCompleted;

        public int PendingCount => _pendingQueue.Count;
        public bool HasActiveBlockingEvent => _activeBlockingEvent.HasValue;
        public bool IsNativeConnected => _nativeAvailable && _nativeHandle != IntPtr.Zero;
        public float PlaybackSpeedMultiplier
        {
            get => playbackSpeedMultiplier;
            set => playbackSpeedMultiplier = Mathf.Clamp(value, 0.25f, 8.0f);
        }

        private void Awake()
        {
            TryInitializeNativeBridge();
        }

        private void OnDestroy()
        {
            ShutdownNativeBridge();
        }

        private void Update()
        {
            if (pollNativeEngineOnUpdate && IsNativeConnected)
            {
                PollNativeEvents();
            }

            TickSequencer(Time.deltaTime * Mathf.Max(0.25f, playbackSpeedMultiplier));
        }

        /// <summary>
        /// Attempts to initialize a native match handle via RustEngineBridge P/Invoke.
        /// Gracefully falls back to managed queue mode if the .dylib/.dll/.so is not yet built.
        /// </summary>
        public bool TryInitializeNativeBridge()
        {
            if (IsNativeConnected) return true;
            try
            {
                _nativeHandle = string.IsNullOrEmpty(_customDeckJson)
                    ? RustEngineBridge.NativeCreateMatch(initialSeed, starterSeat)
                    : RustEngineBridge.NativeCreateMatchWithScenario(initialSeed, starterSeat, _customDeckJson, _customStage);
                _nativeAvailable = _nativeHandle != IntPtr.Zero;
                return _nativeAvailable;
            }
            catch (DllNotFoundException)
            {
                _nativeAvailable = false;
                _nativeHandle = IntPtr.Zero;
                return false;
            }
            catch (EntryPointNotFoundException)
            {
                _nativeAvailable = false;
                _nativeHandle = IntPtr.Zero;
                return false;
            }
        }

        public bool StartCustomDeck(IReadOnlyList<string> cardIds, ulong? seed = null, uint stage = 0)
        {
            if (cardIds == null || cardIds.Count != 30 || stage > 3) return false;
            var json = new StringBuilder("[");
            for (int i = 0; i < cardIds.Count; i++)
            {
                string id = cardIds[i];
                if (string.IsNullOrEmpty(id)) return false;
                foreach (char c in id) if (!(char.IsLetterOrDigit(c) || c == '-')) return false;
                if (i > 0) json.Append(',');
                json.Append('"').Append(id).Append('"');
            }
            json.Append(']');
            string candidate = json.ToString();
            IntPtr handle;
            ulong matchSeed = seed ?? initialSeed;
            try { handle = RustEngineBridge.NativeCreateMatchWithScenario(matchSeed, starterSeat, candidate, stage); }
            catch (DllNotFoundException) { return false; }
            catch (EntryPointNotFoundException) { return false; }
            if (handle == IntPtr.Zero) return false;
            ShutdownNativeBridge();
            initialSeed = matchSeed;
            _customDeckJson = candidate;
            _customStage = stage;
            _nativeHandle = handle;
            _nativeAvailable = true;
            return true;
        }

        public void ShutdownNativeBridge()
        {
            if (_nativeHandle != IntPtr.Zero)
            {
                try
                {
                    RustEngineBridge.NativeDestroyMatch(_nativeHandle);
                }
                catch (Exception)
                {
                    // Ignore shutdown errors if library unloaded
                }
                _nativeHandle = IntPtr.Zero;
            }
            _nativeAvailable = false;
            while (_pendingQueue.TryDequeue(out _)) { }
            _activeBlockingEvent = null;
            _activeBlockingRemainingTime = 0f;
        }

        public bool ReseedAndReinitializeBridge(ulong seedDelta = 1UL)
        {
            ShutdownNativeBridge();
            initialSeed = (initialSeed + Math.Max(1UL, seedDelta)) % 2147483647UL;
            return TryInitializeNativeBridge();
        }

        /// <summary>
        /// Submits a serialized JSON GameCommand to the Rust engine and drains resulting VisualEvents.
        /// </summary>
        public string SubmitCommandJson(string commandJson)
        {
            if (string.IsNullOrWhiteSpace(commandJson))
            {
                return "{\"status\":\"error\",\"reason\":\"empty_command\"}";
            }

            if (IsNativeConnected)
            {
                IntPtr respPtr = RustEngineBridge.NativeSubmitCommandJson(_nativeHandle, commandJson);
                string response = RustEngineBridge.ConsumeNativeString(respPtr);
                PollNativeEvents();
                return response;
            }

            return "{\"status\":\"stub_ok\",\"mode\":\"managed_fallback\"}";
        }

        /// <summary>
        /// Enqueues a single struct event directly into the visual event queue.
        /// </summary>
        public void EnqueueEvent(VisualEventRecord evt)
        {
            if (evt.duration <= 0f)
            {
                evt.duration = DefaultDurationFor(evt.eventType);
            }
            _pendingQueue.Enqueue(evt);
        }

        /// <summary>
        /// Parses and enqueues a JSON batch payload of VisualEventRecords (`{"events":[...]}` or `[...]`).
        /// </summary>
        public int EnqueueEventsFromJson(string jsonPayload)
        {
            if (string.IsNullOrWhiteSpace(jsonPayload))
            {
                return 0;
            }

            string trimmed = jsonPayload.Trim();
            if (trimmed == "[]")
            {
                return 0;
            }

            string wrapped = trimmed.StartsWith("[", StringComparison.Ordinal)
                ? "{\"events\":" + trimmed + "}"
                : trimmed;

            VisualEventBatchPayload batch = JsonUtility.FromJson<VisualEventBatchPayload>(wrapped);
            if (batch?.events == null || batch.events.Count == 0)
            {
                return 0;
            }

            int count = 0;
            foreach (VisualEventRecord evt in batch.events)
            {
                EnqueueEvent(evt);
                count++;
            }
            return count;
        }

        /// <summary>
        /// Drains any pending JSON events from the native Rust handle into the local queue.
        /// </summary>
        public int PollNativeEvents()
        {
            if (!IsNativeConnected)
            {
                return 0;
            }

            IntPtr jsonPtr = RustEngineBridge.NativeDrainVisualEventsJson(_nativeHandle);
            string json = RustEngineBridge.ConsumeNativeString(jsonPtr);
            return EnqueueEventsFromJson(json);
        }

        /// <summary>
        /// Advances active blocking animations and drains non-blocking events until the next blocking event.
        /// </summary>
        public void TickSequencer(float deltaTime)
        {
            if (_activeBlockingEvent.HasValue)
            {
                _activeBlockingRemainingTime -= deltaTime;
                if (_activeBlockingRemainingTime <= 0f)
                {
                    VisualEventRecord finished = _activeBlockingEvent.Value;
                    _activeBlockingEvent = null;
                    _activeBlockingRemainingTime = 0f;
                    _completedEvents.Add(finished);
                    OnBlockingEventCompleted?.Invoke(finished);
                }
                else
                {
                    return;
                }
            }

            while (_pendingQueue.TryDequeue(out VisualEventRecord nextEvent))
            {
                OnVisualEventDispatched?.Invoke(nextEvent);
                if (nextEvent.isBlocking)
                {
                    _activeBlockingEvent = nextEvent;
                    _activeBlockingRemainingTime = Mathf.Max(0.01f, nextEvent.duration);
                    break;
                }

                _completedEvents.Add(nextEvent);
            }
        }

        /// <summary>
        /// Returns the current redacted VisualBoardState snapshot JSON from the native Rust engine.
        /// </summary>
        public string GetBoardStateJson()
        {
            if (!IsNativeConnected)
            {
                return string.Empty;
            }
            IntPtr ptr = RustEngineBridge.NativeGetBoardStateJson(_nativeHandle);
            return RustEngineBridge.ConsumeNativeString(ptr);
        }

        /// <summary>
        /// Drains any pending native events and steps the sequencer until both the pending queue
        /// and any active blocking event have completely finished.
        /// </summary>
        public int DrainAllEventsSynchronously(float stepDelta = 0.1f, int maxIterations = 500)
        {
            PollNativeEvents();
            int steps = 0;
            while ((_pendingQueue.Count > 0 || _activeBlockingEvent.HasValue) && steps < maxIterations)
            {
                TickSequencer(Mathf.Max(0.02f, stepDelta));
                steps++;
            }
            return steps;
        }

        private static float DefaultDurationFor(string eventType)
        {
            switch (eventType)
            {
                case "TurnStarted":
                case "TurnChanged": return 0.45f;
                case "CardDrawn":
                case "CardDealt": return 0.25f;
                case "ManaSpent": return 0.25f;
                case "CardPlayed":
                case "UnitPlaced":
                case "UnitSummoned":
                case "ShieldPlaced": return 0.35f;
                case "SpellCast": return 0.40f;
                case "AttackDeclared":
                case "CombatResolved":
                case "DamageDealt": return 0.38f;
                case "ShieldRevealed":
                case "ShieldBroken": return 0.45f;
                case "UnitBounced": return 0.32f;
                case "CardRecovered": return 0.30f;
                case "CardDestroyed":
                case "UnitDestroyed": return 0.35f;
                case "EffectCancelled": return 0.40f;
                case "ComboLostSupport": return 0.35f;
                case "MatchWon":
                case "GameOver": return 0.50f;
                case "BannerMessage": return 0.45f;
                default: return 0.30f;
            }
        }
    }
}

