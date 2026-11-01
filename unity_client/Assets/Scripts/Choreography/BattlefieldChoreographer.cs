using System;
using System.Collections;
using System.Collections.Generic;
using Heav3ns.Board;
using Heav3ns.Input;
using Heav3ns.UI;
using Heav3ns.UnityBridge;
using Heav3ns.Visuals;
using UnityEngine;

namespace Heav3ns.Choreography
{
    [Serializable]
    public class SerializedCardDto
    {
        public int uid;
        public string id;
        public string name;
        public string type;
        public int cost;
        public string text;
        public int combo_n;
        public int owner;
        public bool hidden;
    }

    [Serializable]
    public class SerializedPlayerDto
    {
        public string name;
        public int actions;
        public List<SerializedCardDto> hand = new List<SerializedCardDto>();
        public int hand_count;
        public int deck_count;
        public List<SerializedCardDto> shields = new List<SerializedCardDto>();
        public List<SerializedCardDto> field = new List<SerializedCardDto>();
        public List<SerializedCardDto> discard = new List<SerializedCardDto>();
    }

    [Serializable]
    public class SerializedBoardStateDto
    {
        public string status;
        public ulong seed;
        public ulong revision;
        public string generation;
        public int turn_number;
        public int active = -1;
        public string phase;
        public int winner = -1;
        public List<SerializedCardDto> sequence = new List<SerializedCardDto>();
        public List<SerializedPlayerDto> players = new List<SerializedPlayerDto>();
    }

    /// <summary>
    /// Presentation director & VFX choreographer that binds VisualEventQueue to the 6 zone layouts
    /// on EnchantedTableStage, handles CardDrawn/CardDealt, CardPlayed/UnitPlaced/ShieldPlaced,
    /// AttackDeclared/CombatResolved/DamageDealt, ShieldRevealed/ShieldBroken (elevating shield card
    /// toward camera center, flipping 180 deg to reveal face for 0.45s, triggering crystalline shatter
    /// particles), CardDestroyed/UnitDestroyed (noise-driven dissolve _DissolveAmount 0.0 -> 1.0 before
    /// destroying), ActionHistoryLog recording, and TurnChanged/GameOver.
    /// </summary>
    [DisallowMultipleComponent]
    public class BattlefieldChoreographer : MonoBehaviour
    {
        private class DissolveAnimationEntry
        {
            public CardVisual card;
            public float elapsed;
            public float duration;
        }

        [Header("Core References")]
        [SerializeField] private VisualEventQueue eventQueue;
        [SerializeField] private CardVisual cardPrefab;
        [SerializeField] private Camera mainCamera;
        [SerializeField] private CardInteractionController interactionController;
        [SerializeField] private DiegeticTabletopHud tabletopHud;
        [SerializeField] private TabletopAudioAndProps audioAndProps;
        [SerializeField] private MulliganController mulliganController;
        [SerializeField] private CardInspectorOverlay inspectorOverlay;
        [SerializeField] private ActionHistoryLog actionHistoryLog;
        [SerializeField] private TitleStageController titleStageController;

        [Header("6 Stage Zone Layouts")]
        [SerializeField] private HandArcLayout aliceHandAnchor;
        [SerializeField] private BoardRowLayout aliceSequenceRow;
        [SerializeField] private BoardRowLayout aliceShieldRow;
        [SerializeField] private HandArcLayout whiteRabbitHandAnchor;
        [SerializeField] private BoardRowLayout whiteRabbitSequenceRow;
        [SerializeField] private BoardRowLayout whiteRabbitShieldRow;

        [Header("Camera Trauma & Shake")]
        [SerializeField] private Vector3 baseCameraPosition = new Vector3(0f, 11.5f, -7.8f);
        [SerializeField] private Vector3 baseCameraEuler = new Vector3(56f, 0f, 0f);
        [SerializeField] private float traumaDecayRate = 1.9f;
        [SerializeField] private float maxShakeOffset = 0.38f;
        [SerializeField] private float maxShakeAngle = 2.6f;

        private readonly Dictionary<int, CardVisual> _activeCardsByUid = new Dictionary<int, CardVisual>();
        private readonly List<DissolveAnimationEntry> _dissolvingCards = new List<DissolveAnimationEntry>();
        private CardVisual _activeShieldRevealCard;
        private float _shieldRevealRemainingTime;
        private ParticleSystem _boardDustParticles;
        private ParticleSystem _combatImpactParticles;
        private ParticleSystem _crystallineShatterParticles;
        private float _cameraTrauma;
        private Vector3 _cameraRecoilOffset;
        private float _shakeSeed;
        private bool _subscribed;
        private int _completedDissolveCount;
        private int _shieldBreakDramaCount;

        public IReadOnlyDictionary<int, CardVisual> ActiveCardsByUid => _activeCardsByUid;
        public HandArcLayout AliceHandAnchor => aliceHandAnchor;
        public BoardRowLayout AliceSequenceRow => aliceSequenceRow;
        public BoardRowLayout AliceShieldRow => aliceShieldRow;
        public HandArcLayout WhiteRabbitHandAnchor => whiteRabbitHandAnchor;
        public BoardRowLayout WhiteRabbitSequenceRow => whiteRabbitSequenceRow;
        public BoardRowLayout WhiteRabbitShieldRow => whiteRabbitShieldRow;
        public MulliganController Mulligan => mulliganController;
        public CardInspectorOverlay InspectorOverlay => inspectorOverlay;
        public ActionHistoryLog HistoryLog => actionHistoryLog;
        public TitleStageController TitleStage => titleStageController;
        public float CameraTrauma => _cameraTrauma;
        public int CompletedDissolveCount => _completedDissolveCount;
        public int ShieldBreakDramaCount => _shieldBreakDramaCount;
        public CardVisual ActiveShieldRevealCard => _activeShieldRevealCard;
        public int ActiveDissolvingCardCount => _dissolvingCards.Count;

        private void Awake()
        {
            EnsureReferences();
            SubscribeEvents();
        }

        private void Start()
        {
            EnsureReferences();
            SubscribeEvents();
            if (Application.isPlaying && eventQueue != null && eventQueue.IsNativeConnected)
            {
                SyncBoardStateFromNative(true);
            }
        }

        private void OnDestroy()
        {
            UnsubscribeEvents();
        }

        public void EnsureReferences()
        {
            if (eventQueue == null)
            {
                eventQueue = GetComponent<VisualEventQueue>();
                if (eventQueue == null) eventQueue = FindAnyObjectByType<VisualEventQueue>();
            }
            if (mainCamera == null)
            {
                mainCamera = Camera.main != null ? Camera.main : FindAnyObjectByType<Camera>();
            }
            if (interactionController == null)
            {
                interactionController = FindAnyObjectByType<CardInteractionController>();
            }
            if (tabletopHud == null)
            {
                tabletopHud = FindAnyObjectByType<DiegeticTabletopHud>();
            }
            if (audioAndProps == null)
            {
                audioAndProps = FindAnyObjectByType<TabletopAudioAndProps>();
            }
            if (mulliganController == null)
            {
                mulliganController = GetComponent<MulliganController>();
                if (mulliganController == null) mulliganController = FindAnyObjectByType<MulliganController>();
            }
            if (inspectorOverlay == null)
            {
                inspectorOverlay = FindAnyObjectByType<CardInspectorOverlay>();
            }
            if (actionHistoryLog == null)
            {
                actionHistoryLog = FindAnyObjectByType<ActionHistoryLog>();
            }
            if (titleStageController == null)
            {
                titleStageController = FindAnyObjectByType<TitleStageController>();
            }

            GameObject stage = GameObject.Find("EnchantedTableStage");
            if (stage != null)
            {
                if (aliceHandAnchor == null) aliceHandAnchor = GetOrAddHandLayout(stage.transform, "AliceHandAnchor", 0, false);
                if (aliceSequenceRow == null) aliceSequenceRow = GetOrAddRowLayout(stage.transform, "AliceSequenceRow", 0, BoardRowLayout.RowKind.Sequence, false);
                if (aliceShieldRow == null) aliceShieldRow = GetOrAddRowLayout(stage.transform, "AliceShieldRow", 0, BoardRowLayout.RowKind.Shield, true);
                if (whiteRabbitHandAnchor == null) whiteRabbitHandAnchor = GetOrAddHandLayout(stage.transform, "WhiteRabbitHandAnchor", 1, true);
                if (whiteRabbitSequenceRow == null) whiteRabbitSequenceRow = GetOrAddRowLayout(stage.transform, "WhiteRabbitSequenceRow", 1, BoardRowLayout.RowKind.Sequence, false);
                if (whiteRabbitShieldRow == null) whiteRabbitShieldRow = GetOrAddRowLayout(stage.transform, "WhiteRabbitShieldRow", 1, BoardRowLayout.RowKind.Shield, true);
            }

            EnsureVfxSystems();
            SubscribeEvents();
        }

        public void BindReferences(
            VisualEventQueue queue,
            CardVisual prefab,
            Camera cam,
            CardInteractionController interaction,
            DiegeticTabletopHud hud,
            TabletopAudioAndProps audio,
            HandArcLayout aliceHand,
            BoardRowLayout aliceSeq,
            BoardRowLayout aliceShield,
            HandArcLayout rabbitHand,
            BoardRowLayout rabbitSeq,
            BoardRowLayout rabbitShield,
            MulliganController mulligan = null,
            CardInspectorOverlay inspector = null,
            ActionHistoryLog history = null,
            TitleStageController titleStage = null)
        {
            eventQueue = queue;
            cardPrefab = prefab;
            mainCamera = cam;
            interactionController = interaction;
            tabletopHud = hud;
            audioAndProps = audio;
            aliceHandAnchor = aliceHand;
            aliceSequenceRow = aliceSeq;
            aliceShieldRow = aliceShield;
            whiteRabbitHandAnchor = rabbitHand;
            whiteRabbitSequenceRow = rabbitSeq;
            whiteRabbitShieldRow = rabbitShield;
            if (mulligan != null) mulliganController = mulligan;
            if (inspector != null) inspectorOverlay = inspector;
            if (history != null) actionHistoryLog = history;
            if (titleStage != null) titleStageController = titleStage;
            EnsureVfxSystems();
            SubscribeEvents();
        }

        private static HandArcLayout GetOrAddHandLayout(Transform stage, string name, int seat, bool faceDown)
        {
            Transform t = stage.Find(name);
            if (t == null) return null;
            HandArcLayout layout = t.GetComponent<HandArcLayout>();
            if (layout == null) layout = t.gameObject.AddComponent<HandArcLayout>();
            layout.ConfigureHandZone(seat, faceDown);
            return layout;
        }

        private static BoardRowLayout GetOrAddRowLayout(Transform stage, string name, int seat, BoardRowLayout.RowKind kind, bool faceDownShields)
        {
            Transform t = stage.Find(name);
            if (t == null) return null;
            BoardRowLayout layout = t.GetComponent<BoardRowLayout>();
            if (layout == null) layout = t.gameObject.AddComponent<BoardRowLayout>();
            layout.ConfigureRow(seat, kind, faceDownShields);
            return layout;
        }

        private void EnsureVfxSystems()
        {
            if (_boardDustParticles == null)
            {
                Transform existingDust = transform.Find("BoardDustParticles");
                if (existingDust != null)
                {
                    _boardDustParticles = existingDust.GetComponent<ParticleSystem>();
                }
                else
                {
                    _boardDustParticles = CreateBurstParticleSystem("BoardDustParticles", new Color(0.88f, 0.82f, 0.68f, 0.75f), 2.4f, 0.22f);
                }
            }

            if (_combatImpactParticles == null)
            {
                Transform existingImpact = transform.Find("CombatImpactParticles");
                if (existingImpact != null)
                {
                    _combatImpactParticles = existingImpact.GetComponent<ParticleSystem>();
                }
                else
                {
                    _combatImpactParticles = CreateBurstParticleSystem("CombatImpactParticles", new Color(1.0f, 0.54f, 0.22f, 0.95f), 5.2f, 0.28f);
                }
            }

            if (_crystallineShatterParticles == null)
            {
                Transform existingShatter = transform.Find("CrystallineShatterParticles");
                if (existingShatter != null)
                {
                    _crystallineShatterParticles = existingShatter.GetComponent<ParticleSystem>();
                }
                else
                {
                    _crystallineShatterParticles = CreateBurstParticleSystem("CrystallineShatterParticles", new Color(0.58f, 0.92f, 1.0f, 0.98f), 6.8f, 0.24f);
                }
            }
        }

        private ParticleSystem CreateBurstParticleSystem(string name, Color color, float speed, float size)
        {
            GameObject go = new GameObject(name);
            go.transform.SetParent(transform, false);
            ParticleSystem ps = go.AddComponent<ParticleSystem>();
            var main = ps.main;
            main.playOnAwake = false;
            main.loop = false;
            main.duration = 0.35f;
            main.startLifetime = 0.4f;
            main.startSpeed = speed;
            main.startSize = size;
            main.startColor = color;
            var emission = ps.emission;
            emission.enabled = false;
            ps.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
            return ps;
        }

        private void SubscribeEvents()
        {
            if (_subscribed)
            {
                return;
            }
            if (eventQueue != null)
            {
                eventQueue.OnVisualEventDispatched += HandleVisualEventDispatched;
                eventQueue.OnBlockingEventCompleted += HandleBlockingEventCompleted;
                _subscribed = true;
            }
            if (tabletopHud != null)
            {
                tabletopHud.OnEndTurnRequested -= HandleEndTurnRequested;
                tabletopHud.OnEndTurnRequested += HandleEndTurnRequested;
                tabletopHud.OnMatchResetRequested -= HandleMatchResetRequested;
                tabletopHud.OnMatchResetRequested += HandleMatchResetRequested;
            }
        }

        private void UnsubscribeEvents()
        {
            if (eventQueue != null)
            {
                eventQueue.OnVisualEventDispatched -= HandleVisualEventDispatched;
                eventQueue.OnBlockingEventCompleted -= HandleBlockingEventCompleted;
            }
            if (tabletopHud != null)
            {
                tabletopHud.OnEndTurnRequested -= HandleEndTurnRequested;
                tabletopHud.OnMatchResetRequested -= HandleMatchResetRequested;
            }
            _subscribed = false;
        }

        private void Update()
        {
            TickCameraShake(Time.deltaTime);
            TickDissolveAndShieldDrama(Time.deltaTime);
        }

        public void TickCameraShake(float deltaTime)
        {
            if (mainCamera == null)
            {
                return;
            }

            // If TitleStageController is actively showing the title screen, let its camera spring control position
            if (titleStageController != null && titleStageController.IsTitleActive)
            {
                return;
            }

            // If the camera has a SecondOrderSpring still settling from the Title -> Gameplay transition and no trauma is active, let the spring finish smoothly
            if (_cameraTrauma <= 0.001f && _cameraRecoilOffset.sqrMagnitude <= 0.0001f && titleStageController != null && titleStageController.CameraSpring != null)
            {
                if (Vector3.Distance(mainCamera.transform.localPosition, titleStageController.CameraSpring.TargetPosition) > 0.02f)
                {
                    return;
                }
            }

            _shakeSeed += deltaTime * 28f;
            _cameraTrauma = Mathf.Max(0f, _cameraTrauma - traumaDecayRate * deltaTime);
            _cameraRecoilOffset = Vector3.Lerp(_cameraRecoilOffset, Vector3.zero, 1f - Mathf.Exp(-14f * deltaTime));

            float shake = _cameraTrauma * _cameraTrauma;
            Vector3 shakePos = new Vector3(
                (Mathf.PerlinNoise(_shakeSeed, 0.1f) - 0.5f) * 2f * maxShakeOffset * shake,
                (Mathf.PerlinNoise(0.2f, _shakeSeed) - 0.5f) * 2f * maxShakeOffset * shake,
                (Mathf.PerlinNoise(_shakeSeed, _shakeSeed) - 0.5f) * 2f * maxShakeOffset * shake);

            float rollShake = (Mathf.PerlinNoise(_shakeSeed * 1.3f, 0.7f) - 0.5f) * 2f * maxShakeAngle * shake;

            mainCamera.transform.position = baseCameraPosition + _cameraRecoilOffset + shakePos;
            mainCamera.transform.rotation = Quaternion.Euler(baseCameraEuler.x, baseCameraEuler.y, baseCameraEuler.z + rollShake);
        }

        /// <summary>
        /// Steps active noise-driven burn/dissolve animations (_DissolveAmount 0.0 -> 1.0) on destroyed
        /// cards/shields and manages the 0.45s Shield Break camera-center reveal drama before destroying.
        /// </summary>
        public void TickDissolveAndShieldDrama(float deltaTime)
        {
            if (deltaTime <= 0f)
            {
                return;
            }

            if (_activeShieldRevealCard != null)
            {
                _activeShieldRevealCard.Spring.Step(deltaTime);
                _shieldRevealRemainingTime -= deltaTime;
                if (_shieldRevealRemainingTime <= 0f)
                {
                    CardVisual finishedReveal = _activeShieldRevealCard;
                    _activeShieldRevealCard = null;
                    RouteRevealedShieldPerBoardState(finishedReveal);
                }
            }

            for (int i = _dissolvingCards.Count - 1; i >= 0; i--)
            {
                DissolveAnimationEntry entry = _dissolvingCards[i];
                if (entry.card == null)
                {
                    _dissolvingCards.RemoveAt(i);
                    continue;
                }

                entry.elapsed += deltaTime;
                float t = Mathf.Clamp01(entry.elapsed / Mathf.Max(0.01f, entry.duration));
                entry.card.SetDissolveAmount(t);

                if (t >= 1f)
                {
                    _completedDissolveCount++;
                    DestroyCardActor(entry.card);
                    _dissolvingCards.RemoveAt(i);
                }
            }
        }

        private void RouteRevealedShieldPerBoardState(CardVisual revealedCard)
        {
            if (revealedCard == null)
            {
                return;
            }

            // Query heav3ns_engine_get_board_state_json to see if the revealed shield card moved to hand or graveyard/discard
            if (eventQueue != null && eventQueue.IsNativeConnected && revealedCard.CardUid > 0)
            {
                string boardJson = eventQueue.GetBoardStateJson();
                if (!string.IsNullOrWhiteSpace(boardJson))
                {
                    string normalizedJson = boardJson
                        .Replace("\"active\":null", "\"active\":-1")
                        .Replace("\"winner\":null", "\"winner\":-1");
                    SerializedBoardStateDto state = JsonUtility.FromJson<SerializedBoardStateDto>(normalizedJson);
                    if (state?.players != null && revealedCard.OwnerSeat >= 0 && revealedCard.OwnerSeat < state.players.Count)
                    {
                        SerializedPlayerDto ownerPlayer = state.players[revealedCard.OwnerSeat];
                        bool inHand = ownerPlayer.hand != null && ownerPlayer.hand.Exists(c => c.uid == revealedCard.CardUid);
                        if (inHand)
                        {
                            HandArcLayout targetHand = revealedCard.OwnerSeat == 0 ? aliceHandAnchor : whiteRabbitHandAnchor;
                            if (targetHand != null)
                            {
                                _activeCardsByUid[revealedCard.CardUid] = revealedCard;
                                targetHand.AddCard(revealedCard, false);
                                return;
                            }
                        }
                    }
                }
            }

            // Route to graveyard with noise-driven burn/dissolve
            BeginDissolveAndDestroyCard(revealedCard, 0.30f);
        }

        /// <summary>
        /// Begins a noise-driven burn/dissolve animation (_DissolveAmount 0.0 -> 1.0) on the given CardVisual
        /// before destroying the GameObject.
        /// </summary>
        public void BeginDissolveAndDestroyCard(CardVisual card, float duration = 0.35f)
        {
            if (card == null)
            {
                return;
            }

            for (int i = 0; i < _dissolvingCards.Count; i++)
            {
                if (_dissolvingCards[i].card == card)
                {
                    return;
                }
            }

            RemoveCardFromAllZones(card);
            if (card.CardUid != 0)
            {
                _activeCardsByUid.Remove(card.CardUid);
            }

            card.transform.SetParent(transform, true);
            card.SetDissolveAmount(0.05f);
            _dissolvingCards.Add(new DissolveAnimationEntry
            {
                card = card,
                elapsed = 0.01f,
                duration = Mathf.Max(0.05f, duration)
            });
        }

        /// <summary>
        /// Phase 2 High-Impact Shield Break & Reveal Drama:
        /// Elevates the targeted shield card toward the camera center, flips it 180 degrees to reveal
        /// its face to the player for 0.45s, triggers a crystalline shatter particle burst, and routes
        /// the card to hand/graveyard or dissolves it.
        /// </summary>
        public CardVisual TriggerShieldBreakDrama(VisualEventRecord evt)
        {
            EnsureReferences();
            _shieldBreakDramaCount++;

            if (audioAndProps != null)
            {
                audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.ShieldShatter);
            }

            BoardRowLayout shieldRow = evt.seat == 0 ? aliceShieldRow : whiteRabbitShieldRow;
            CardVisual shieldCard = null;

            if (evt.cardUid > 0 && _activeCardsByUid.TryGetValue(evt.cardUid, out CardVisual tracked) && tracked != null)
            {
                shieldCard = tracked;
            }
            else if (shieldRow != null && shieldRow.Cards.Count > 0)
            {
                shieldCard = shieldRow.Cards[shieldRow.Cards.Count - 1];
            }

            if (shieldCard == null)
            {
                shieldCard = InstantiateCardActor();
            }
            else
            {
                RemoveCardFromAllZones(shieldCard);
            }

            if (_activeShieldRevealCard != null && _activeShieldRevealCard != shieldCard)
            {
                BeginDissolveAndDestroyCard(_activeShieldRevealCard, 0.25f);
            }

            shieldCard.transform.SetParent(transform, true);
            shieldCard.ConfigureCard(
                evt.cardUid != 0 ? evt.cardUid : -4000 - _shieldBreakDramaCount,
                !string.IsNullOrEmpty(evt.cardSpecId) ? evt.cardSpecId : "H3W-AL-001",
                evt.seat,
                !string.IsNullOrEmpty(evt.title) ? evt.title : "Revealed Shield",
                !string.IsNullOrEmpty(evt.cardType) ? evt.cardType : "Action",
                Mathf.Max(1, evt.cost),
                !string.IsNullOrEmpty(evt.triggerText) ? $"Shield Trigger: {evt.triggerText}" : evt.text,
                Mathf.Max(1, evt.cost),
                Mathf.Max(1, evt.cost + 1),
                false,
                "shield_reveal",
                false);

            // Start flipped 180 deg, then elevate toward camera center and flip 180 deg to reveal face for 0.45s
            Vector3 revealCenterWorld = new Vector3(0f, 4.45f, -3.25f);
            shieldCard.transform.localRotation = Quaternion.Euler(26f, 0f, 180f);
            shieldCard.Spring.SetTarget(
                transform.InverseTransformPoint(revealCenterWorld),
                Quaternion.Euler(26f, 0f, 0f),
                Vector3.one * 1.45f,
                false);
            shieldCard.Spring.TriggerImpactSquashPunch();

            EmitCrystallineShatterAt(revealCenterWorld);
            FloatingCombatText.Spawn(revealCenterWorld + Vector3.up * 0.4f, "SHIELD REVEALED!", new Color(0.55f, 0.94f, 1.0f, 1f), transform);
            AddCameraTrauma(0.52f);
            ApplyCameraRecoil(new Vector3(0f, -0.25f, -0.15f));

            _activeShieldRevealCard = shieldCard;
            _shieldRevealRemainingTime = 0.45f;
            return shieldCard;
        }

        public void AddCameraTrauma(float amount)
        {
            _cameraTrauma = Mathf.Clamp01(_cameraTrauma + Mathf.Max(0f, amount));
        }

        public void ApplyCameraRecoil(Vector3 impulseOffset)
        {
            _cameraRecoilOffset += impulseOffset;
        }

        /// <summary>
        /// Handles each VisualEventRecord emitted by VisualEventQueue:
        /// - CardDrawn / CardDealt
        /// - CardPlayed / UnitPlaced / UnitSummoned / SpellCast / ShieldPlaced
        /// - AttackDeclared / CombatResolved / DamageDealt / ShieldRevealed
        /// - ShieldBroken / CardDestroyed / UnitDestroyed / UnitBounced
        /// - TurnChanged / TurnStarted / GameOver / MatchWon
        /// </summary>
        public void HandleVisualEventDispatched(VisualEventRecord evt)
        {
            EnsureReferences();

            if (actionHistoryLog != null)
            {
                actionHistoryLog.RecordVisualEvent(evt);
            }

            switch (evt.eventType)
            {
                case "CardDrawn":
                case "CardDealt":
                {
                    if (audioAndProps != null)
                    {
                        audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.CardDrawSnap);
                    }

                    HandArcLayout hand = evt.seat == 0 ? aliceHandAnchor : whiteRabbitHandAnchor;
                    if (hand != null)
                    {
                        CardVisual drawn = GetOrCreateCardVisual(
                            evt.cardUid,
                            evt.cardSpecId,
                            evt.seat,
                            evt.title,
                            evt.cardType,
                            evt.cost,
                            evt.text,
                            evt.seat != 0,
                            "hand");

                        // Spawn at deck origin on the right side of the board and flip/spring into hand
                        Vector3 deckOriginWorld = new Vector3(6.5f, 0.6f, evt.seat == 0 ? -3.4f : 3.4f);
                        drawn.transform.position = deckOriginWorld;
                        drawn.transform.rotation = Quaternion.Euler(0f, evt.seat == 0 ? 0f : 180f, 180f);
                        hand.AddCard(drawn, false);
                    }
                    break;
                }

                case "ManaSpent":
                {
                    Vector3 spawnPos = evt.seat == 0 ? new Vector3(-5.8f, 0.4f, -3.5f) : new Vector3(-5.8f, 0.4f, 3.5f);
                    FloatingCombatText.Spawn(spawnPos, $"-{evt.amount} MANA", new Color(0.45f, 0.88f, 1.0f, 1f), transform);
                    break;
                }

                case "CardPlayed":
                case "UnitPlaced":
                case "UnitSummoned":
                case "SpellCast":
                case "ShieldPlaced":
                {
                    if (audioAndProps != null)
                    {
                        audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.CardboardBoardThud);
                    }

                    HandArcLayout sourceHand = evt.seat == 0 ? aliceHandAnchor : whiteRabbitHandAnchor;
                    bool toShield = evt.eventType == "ShieldPlaced";
                    BoardRowLayout targetRow = toShield
                        ? (evt.seat == 0 ? aliceShieldRow : whiteRabbitShieldRow)
                        : (evt.seat == 0 ? aliceSequenceRow : whiteRabbitSequenceRow);

                    CardVisual card = FindOrPromoteCardForPlay(evt, sourceHand);
                    if (card != null && targetRow != null)
                    {
                        if (sourceHand != null)
                        {
                            sourceHand.RemoveCard(card);
                        }
                        card.SetFaceDown(toShield);
                        targetRow.AddCard(card, evt.slotIndex, false, true);

                        EmitDustBurstAt(card.transform.position);
                        ApplyCameraRecoil(new Vector3(0f, -0.16f, -0.08f));
                        AddCameraTrauma(0.22f);
                    }
                    break;
                }

                case "ShieldRevealed":
                {
                    TriggerShieldBreakDrama(evt);
                    break;
                }

                case "AttackDeclared":
                case "CombatResolved":
                case "DamageDealt":
                {
                    if (audioAndProps != null)
                    {
                        audioAndProps.PlaySound(evt.isShieldBreak
                            ? TabletopAudioAndProps.SoundEffectType.ShieldShatter
                            : TabletopAudioAndProps.SoundEffectType.CardboardBoardThud);
                    }

                    Vector3 impactPos = evt.seat == 0
                        ? (aliceShieldRow != null ? aliceShieldRow.transform.position : new Vector3(-5.2f, 0.3f, -2.8f))
                        : (whiteRabbitShieldRow != null ? whiteRabbitShieldRow.transform.position : new Vector3(5.2f, 0.3f, 2.8f));

                    // Wind back attacker and lunge rapidly toward defender transform
                    BoardRowLayout attackerRow = evt.seat == 0 ? whiteRabbitSequenceRow : aliceSequenceRow;
                    if (attackerRow != null && attackerRow.Cards.Count > 0)
                    {
                        CardVisual attacker = attackerRow.Cards[attackerRow.Cards.Count - 1];
                        if (attacker != null && attacker.Spring != null)
                        {
                            Vector3 lungeDir = (impactPos - attacker.transform.position).normalized;
                            attacker.Spring.AddImpulse(lungeDir * 9.5f + Vector3.up * 2.2f);
                            attacker.PulseStatBadges();
                        }
                    }

                    EmitCombatImpactAt(impactPos);
                    if (evt.isShieldBreak)
                    {
                        EmitCrystallineShatterAt(impactPos);
                    }
                    int dmgAmount = Mathf.Max(1, evt.amount);
                    string bannerText = evt.isShieldBreak ? $"-{dmgAmount} SHIELD!" : $"-{dmgAmount}";
                    FloatingCombatText.Spawn(impactPos, bannerText, new Color(1.0f, 0.36f, 0.24f, 1f), transform);
                    AddCameraTrauma(0.48f);
                    ApplyCameraRecoil(new Vector3(0f, -0.25f, -0.15f));
                    break;
                }

                case "ShieldBroken":
                {
                    CardVisual dramaCard = TriggerShieldBreakDrama(evt);
                    if (dramaCard != null)
                    {
                        // Also start dissolve when shield broken
                        BeginDissolveAndDestroyCard(dramaCard, 0.35f);
                        _activeShieldRevealCard = null;
                    }
                    break;
                }

                case "CardDestroyed":
                case "UnitDestroyed":
                case "UnitBounced":
                {
                    if (_activeShieldRevealCard != null && _activeShieldRevealCard.CardUid == evt.cardUid && evt.eventType != "UnitBounced")
                    {
                        // Let the 0.45s shield reveal drama complete before routing/dissolving
                        break;
                    }
                    if (_activeCardsByUid.TryGetValue(evt.cardUid, out CardVisual card) && card != null)
                    {
                        EmitCombatImpactAt(card.transform.position);
                        RemoveCardFromAllZones(card);

                        if (evt.eventType == "UnitBounced")
                        {
                            HandArcLayout ownerHand = evt.seat == 0 ? aliceHandAnchor : whiteRabbitHandAnchor;
                            if (ownerHand != null)
                            {
                                ownerHand.AddCard(card, false);
                            }
                        }
                        else
                        {
                            BeginDissolveAndDestroyCard(card, 0.35f);
                        }
                    }
                    break;
                }

                case "TurnStarted":
                case "TurnChanged":
                {
                    string title = string.IsNullOrEmpty(evt.text) ? $"TURN {Mathf.Max(1, evt.turnNumber)}" : evt.text.ToUpperInvariant();
                    string sub = string.IsNullOrEmpty(evt.subtext) ? (evt.seat == 0 ? "Alice's Turn" : "White Rabbit's Turn") : evt.subtext;
                    if (tabletopHud != null)
                    {
                        tabletopHud.ShowBanner(title, sub, evt.duration > 0f ? evt.duration : 0.55f);
                    }
                    if (interactionController != null && (mulliganController == null || !mulliganController.IsMulliganActive))
                    {
                        interactionController.InteractionEnabled = (evt.seat == 0);
                    }
                    break;
                }

                case "MatchWon":
                case "GameOver":
                {
                    string winnerText = string.IsNullOrEmpty(evt.text) ? (evt.seat == 0 ? "PLAYER WINS!" : "OPPONENT WINS!") : evt.text;
                    if (tabletopHud != null)
                    {
                        tabletopHud.SetGameOverState(true);
                        tabletopHud.ShowBanner("MATCH OVER", winnerText, 5.0f);
                    }
                    if (interactionController != null)
                    {
                        interactionController.InteractionEnabled = false;
                    }
                    AddCameraTrauma(0.6f);
                    break;
                }
            }
        }

        private void HandleBlockingEventCompleted(VisualEventRecord evt)
        {
            if (eventQueue != null && eventQueue.PendingCount == 0 && eventQueue.IsNativeConnected)
            {
                SyncBoardStateFromNative(false);
            }
        }

        /// <summary>
        /// Triggers the End Turn command on RustEngineBridge with White Rabbit AI enabled (run_ai: true),
        /// draining and choreographing all resulting visual events.
        /// </summary>
        public void HandleEndTurnRequested()
        {
            if (eventQueue == null)
            {
                return;
            }
            eventQueue.SubmitCommandJson("{\"command\":\"end_turn\",\"run_ai\":true}");
            if (eventQueue.PendingCount == 0)
            {
                SyncBoardStateFromNative(false);
            }
        }

        public void HandleMatchResetRequested()
        {
            if (eventQueue == null)
            {
                return;
            }
            ClearAllZones();
            if (actionHistoryLog != null)
            {
                actionHistoryLog.ClearLog();
            }
            if (inspectorOverlay != null)
            {
                inspectorOverlay.HideInspection();
            }
            eventQueue.ShutdownNativeBridge();
            eventQueue.TryInitializeNativeBridge();
            eventQueue.PollNativeEvents();
            SyncBoardStateFromNative(true);
        }

        public void AdoptInitializedMatch()
        {
            ClearAllZones();
            if (actionHistoryLog != null) actionHistoryLog.ClearLog();
            if (inspectorOverlay != null) inspectorOverlay.HideInspection();
            if (eventQueue != null && eventQueue.IsNativeConnected)
            {
                eventQueue.PollNativeEvents();
                SyncBoardStateFromNative(true);
            }
        }

        private CardVisual FindOrPromoteCardForPlay(VisualEventRecord evt, HandArcLayout sourceHand)
        {
            if (evt.cardUid > 0 && _activeCardsByUid.TryGetValue(evt.cardUid, out CardVisual existing) && existing != null)
            {
                existing.ConfigureCard(
                    evt.cardUid,
                    evt.cardSpecId,
                    evt.seat,
                    evt.title,
                    evt.cardType,
                    evt.cost,
                    evt.text,
                    Mathf.Max(1, evt.cost),
                    Mathf.Max(1, evt.cost + 1),
                    false,
                    "field",
                    true);
                return existing;
            }

            // If an opponent hidden card in hand is played, promote the first face-down card in their hand
            if (sourceHand != null && sourceHand.Cards.Count > 0)
            {
                CardVisual promoted = sourceHand.Cards[0];
                sourceHand.RemoveCard(promoted);
                if (promoted != null)
                {
                    promoted.ConfigureCard(
                        evt.cardUid,
                        evt.cardSpecId,
                        evt.seat,
                        evt.title,
                        evt.cardType,
                        evt.cost,
                        evt.text,
                        Mathf.Max(1, evt.cost),
                        Mathf.Max(1, evt.cost + 1),
                        false,
                        "field",
                        true);
                    if (evt.cardUid > 0)
                    {
                        _activeCardsByUid[evt.cardUid] = promoted;
                    }
                    return promoted;
                }
            }

            return GetOrCreateCardVisual(
                evt.cardUid,
                evt.cardSpecId,
                evt.seat,
                evt.title,
                evt.cardType,
                evt.cost,
                evt.text,
                false,
                "field");
        }

        public CardVisual GetOrCreateCardVisual(
            int uid,
            string specId,
            int ownerSeat,
            string title,
            string cardType,
            int cost,
            string rulesText,
            bool faceDown,
            string zone,
            bool playable = true)
        {
            if (uid > 0 && _activeCardsByUid.TryGetValue(uid, out CardVisual existing) && existing != null)
            {
                existing.ConfigureCard(
                    uid,
                    specId,
                    ownerSeat,
                    title,
                    cardType,
                    cost,
                    rulesText,
                    Mathf.Max(1, cost),
                    Mathf.Max(1, cost + 1),
                    faceDown,
                    zone,
                    playable);
                return existing;
            }

            CardVisual instance = InstantiateCardActor();
            instance.ConfigureCard(
                uid,
                specId,
                ownerSeat,
                title,
                cardType,
                cost,
                rulesText,
                Mathf.Max(1, cost),
                Mathf.Max(1, cost + 1),
                faceDown,
                zone,
                playable);

            if (uid > 0)
            {
                _activeCardsByUid[uid] = instance;
            }
            return instance;
        }

        private CardVisual InstantiateCardActor()
        {
            if (cardPrefab != null)
            {
                CardVisual inst = Instantiate(cardPrefab);
                inst.EnsureReferences();
                return inst;
            }
            return CardVisual.ConstructProceduralCardObject("CardActor");
        }

        private void RemoveCardFromAllZones(CardVisual card)
        {
            if (aliceHandAnchor != null) aliceHandAnchor.RemoveCard(card);
            if (aliceSequenceRow != null) aliceSequenceRow.RemoveCard(card);
            if (aliceShieldRow != null) aliceShieldRow.RemoveCard(card);
            if (whiteRabbitHandAnchor != null) whiteRabbitHandAnchor.RemoveCard(card);
            if (whiteRabbitSequenceRow != null) whiteRabbitSequenceRow.RemoveCard(card);
            if (whiteRabbitShieldRow != null) whiteRabbitShieldRow.RemoveCard(card);
        }

        public void ClearAllZones()
        {
            if (_activeShieldRevealCard != null)
            {
                DestroyCardActor(_activeShieldRevealCard);
                _activeShieldRevealCard = null;
            }
            for (int i = _dissolvingCards.Count - 1; i >= 0; i--)
            {
                if (_dissolvingCards[i].card != null)
                {
                    DestroyCardActor(_dissolvingCards[i].card);
                }
            }
            _dissolvingCards.Clear();

            if (aliceHandAnchor != null) aliceHandAnchor.ClearCards(true);
            if (aliceSequenceRow != null) aliceSequenceRow.ClearCards(true);
            if (aliceShieldRow != null) aliceShieldRow.ClearCards(true);
            if (whiteRabbitHandAnchor != null) whiteRabbitHandAnchor.ClearCards(true);
            if (whiteRabbitSequenceRow != null) whiteRabbitSequenceRow.ClearCards(true);
            if (whiteRabbitShieldRow != null) whiteRabbitShieldRow.ClearCards(true);
            _activeCardsByUid.Clear();
        }

        private static void DestroyCardActor(CardVisual card)
        {
            if (card == null) return;
            if (Application.isPlaying)
            {
                Destroy(card.gameObject);
            }
            else
            {
                DestroyImmediate(card.gameObject);
            }
        }

        private void EmitDustBurstAt(Vector3 worldPos)
        {
            EnsureVfxSystems();
            if (_boardDustParticles != null)
            {
                _boardDustParticles.transform.position = worldPos + Vector3.up * 0.08f;
                _boardDustParticles.Emit(16);
            }
        }

        private void EmitCombatImpactAt(Vector3 worldPos)
        {
            EnsureVfxSystems();
            if (_combatImpactParticles != null)
            {
                _combatImpactParticles.transform.position = worldPos + Vector3.up * 0.25f;
                _combatImpactParticles.Emit(24);
            }
        }

        private void EmitCrystallineShatterAt(Vector3 worldPos)
        {
            EnsureVfxSystems();
            if (_crystallineShatterParticles != null)
            {
                _crystallineShatterParticles.transform.position = worldPos + Vector3.up * 0.20f;
                _crystallineShatterParticles.Emit(32);
            }
        }

        /// <summary>
        /// Queries heav3ns_engine_get_board_state_json via VisualEventQueue and synchronizes all 6
        /// zone layouts (AliceHandAnchor, AliceSequenceRow, AliceShieldRow, WhiteRabbitHandAnchor,
        /// WhiteRabbitSequenceRow, WhiteRabbitShieldRow) and DiegeticTabletopHud counters so they
        /// exactly match the authoritative Rust engine state.
        /// </summary>
        public SerializedBoardStateDto SyncBoardStateFromNative(bool snapImmediately = false)
        {
            EnsureReferences();
            if (eventQueue == null || !eventQueue.IsNativeConnected)
            {
                return null;
            }

            string boardJson = eventQueue.GetBoardStateJson();
            if (string.IsNullOrWhiteSpace(boardJson))
            {
                return null;
            }

            // Normalize nullable active/winner fields for Unity JsonUtility
            string normalizedJson = boardJson
                .Replace("\"active\":null", "\"active\":-1")
                .Replace("\"winner\":null", "\"winner\":-1");

            SerializedBoardStateDto state = JsonUtility.FromJson<SerializedBoardStateDto>(normalizedJson);
            if (state == null || state.players == null || state.players.Count < 2)
            {
                return null;
            }

            ReconcileBoardZones(state, snapImmediately);
            return state;
        }

        public void ReconcileBoardZones(SerializedBoardStateDto state, bool snapImmediately = false)
        {
            if (state == null || state.players == null || state.players.Count < 2)
            {
                return;
            }

            // Clean up any in-flight dissolve/reveal cards on full state reconcile
            if (_activeShieldRevealCard != null)
            {
                DestroyCardActor(_activeShieldRevealCard);
                _activeShieldRevealCard = null;
            }
            for (int i = _dissolvingCards.Count - 1; i >= 0; i--)
            {
                if (_dissolvingCards[i].card != null)
                {
                    _completedDissolveCount++;
                    DestroyCardActor(_dissolvingCards[i].card);
                }
            }
            _dissolvingCards.Clear();

            SerializedPlayerDto alice = state.players[0];
            SerializedPlayerDto rabbit = state.players[1];
            _activeCardsByUid.Clear();

            // 1. Alice Hand (Face-up, playable check based on actions & type)
            if (aliceHandAnchor != null)
            {
                aliceHandAnchor.ClearCards(true);
                foreach (SerializedCardDto c in alice.hand)
                {
                    bool canPlay = state.active == 0 && c.type != "Counter" && c.cost <= alice.actions;
                    CardVisual cv = InstantiateCardActor();
                    cv.ConfigureCard(c.uid, c.id, 0, c.name, c.type, c.cost, c.text, Mathf.Max(1, c.cost), Mathf.Max(1, c.cost + 1), false, "hand", canPlay);
                    _activeCardsByUid[c.uid] = cv;
                    aliceHandAnchor.AddCard(cv, snapImmediately);
                }
            }

            // 2. White Rabbit Hand (Enforce face-down / masked orientation)
            if (whiteRabbitHandAnchor != null)
            {
                whiteRabbitHandAnchor.ClearCards(true);
                int count = rabbit.hand != null && rabbit.hand.Count > 0 ? rabbit.hand.Count : rabbit.hand_count;
                for (int i = 0; i < count; i++)
                {
                    int syntheticUid = -1000 - i;
                    CardVisual cv = InstantiateCardActor();
                    cv.ConfigureCard(syntheticUid, "HIDDEN", 1, "Hidden Card", "Hidden", 0, string.Empty, 0, 1, true, "hand", false);
                    whiteRabbitHandAnchor.AddCard(cv, snapImmediately);
                }
            }

            // 3. Alice Sequence Row (field)
            if (aliceSequenceRow != null)
            {
                aliceSequenceRow.ClearCards(true);
                foreach (SerializedCardDto c in alice.field)
                {
                    CardVisual cv = InstantiateCardActor();
                    cv.ConfigureCard(c.uid, c.id, 0, c.name, c.type, c.cost, c.text, Mathf.Max(1, c.cost), Mathf.Max(1, c.cost + 1), false, "field", false);
                    _activeCardsByUid[c.uid] = cv;
                    aliceSequenceRow.AddCard(cv, -1, snapImmediately, !snapImmediately);
                }
            }

            // 4. White Rabbit Sequence Row (field)
            if (whiteRabbitSequenceRow != null)
            {
                whiteRabbitSequenceRow.ClearCards(true);
                foreach (SerializedCardDto c in rabbit.field)
                {
                    CardVisual cv = InstantiateCardActor();
                    cv.ConfigureCard(c.uid, c.id, 1, c.name, c.type, c.cost, c.text, Mathf.Max(1, c.cost), Mathf.Max(1, c.cost + 1), false, "field", false);
                    _activeCardsByUid[c.uid] = cv;
                    whiteRabbitSequenceRow.AddCard(cv, -1, snapImmediately, !snapImmediately);
                }
            }

            // 5. Alice Shield Row (Face-down unrevealed shields)
            if (aliceShieldRow != null)
            {
                aliceShieldRow.ClearCards(true);
                int shieldCount = alice.shields != null ? alice.shields.Count : 0;
                for (int i = 0; i < shieldCount; i++)
                {
                    int syntheticUid = -2000 - i;
                    CardVisual cv = InstantiateCardActor();
                    cv.ConfigureCard(syntheticUid, "ALICE_SHIELD", 0, "Alice Shield", "Shield", 0, string.Empty, 0, 1, true, "shields", false);
                    aliceShieldRow.AddCard(cv, -1, snapImmediately, !snapImmediately);
                }
            }

            // 6. White Rabbit Shield Row (Face-down unrevealed shields)
            if (whiteRabbitShieldRow != null)
            {
                whiteRabbitShieldRow.ClearCards(true);
                int shieldCount = rabbit.shields != null ? rabbit.shields.Count : 0;
                for (int i = 0; i < shieldCount; i++)
                {
                    int syntheticUid = -3000 - i;
                    CardVisual cv = InstantiateCardActor();
                    cv.ConfigureCard(syntheticUid, "RABBIT_SHIELD", 1, "White Rabbit Shield", "Shield", 0, string.Empty, 0, 1, true, "shields", false);
                    whiteRabbitShieldRow.AddCard(cv, -1, snapImmediately, !snapImmediately);
                }
            }

            // 7. Update Diegetic HUD
            if (tabletopHud != null)
            {
                bool hasPlayable = false;
                if (alice.hand != null)
                {
                    foreach (SerializedCardDto c in alice.hand)
                    {
                        if (c.type != "Counter" && c.cost <= alice.actions)
                        {
                            hasPlayable = true;
                            break;
                        }
                    }
                }

                tabletopHud.UpdatePlayerCounters(
                    state.active,
                    alice.actions,
                    alice.shields != null ? alice.shields.Count : 0,
                    alice.deck_count,
                    alice.discard != null ? alice.discard.Count : 0,
                    rabbit.actions,
                    rabbit.shields != null ? rabbit.shields.Count : 0,
                    rabbit.deck_count,
                    rabbit.discard != null ? rabbit.discard.Count : 0,
                    hasPlayable,
                    state.winner >= 0);
            }
        }
    }
}
