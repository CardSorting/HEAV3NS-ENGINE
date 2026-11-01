using System;
using System.Collections;
using System.Collections.Generic;
using Heav3ns.Board;
using Heav3ns.Input;
using Heav3ns.UI;
using Heav3ns.UnityBridge;
using Heav3ns.Visuals;
using TMPro;
using UnityEngine;

namespace Heav3ns.Choreography
{
    /// <summary>
    /// Phase 2 Opening Mulligan & Shield Dealing Cascade Controller.
    /// At match start (before Turn 1 begins), fans Alice's initial 5 drawn cards across the center
    /// of the camera viewport at 1.6x scale, allows clicking cards to toggle a "Replace / Redraw" badge,
    /// and provides a 3D clickable Confirm Hand button that dispatches the mulligan command (or keeps the hand)
    /// and springs the cards smoothly down into AliceHandAnchor while dealing the 6 Shields into
    /// AliceShieldRow and WhiteRabbitShieldRow with staggered 0.08s cascade delays.
    /// </summary>
    [DisallowMultipleComponent]
    public class MulliganController : MonoBehaviour
    {
        [Header("Mulligan Viewport Layout")]
        [SerializeField] private Vector3 mulliganCenterWorld = new Vector3(0f, 3.85f, -3.15f);
        [SerializeField] private float mulliganCardSpacing = 2.28f;
        [SerializeField] private float mulliganScaleMultiplier = 1.6f;
        [SerializeField] private float shieldDealCascadeDelay = 0.08f;

        [Header("3D Controls & Banner")]
        [SerializeField] private Transform mulliganOverlayRoot;
        [SerializeField] private Transform confirmButtonTransform;
        [SerializeField] private Renderer confirmButtonRenderer;
        [SerializeField] private SecondOrderSpring confirmButtonSpring;
        [SerializeField] private TMP_Text confirmButtonLabel;
        [SerializeField] private TMP_Text mulliganHeaderLabel;

        [Header("Scene References")]
        [SerializeField] private BattlefieldChoreographer choreographer;
        [SerializeField] private VisualEventQueue eventQueue;
        [SerializeField] private CardInteractionController interactionController;
        [SerializeField] private TabletopAudioAndProps audioAndProps;

        private readonly List<CardVisual> _mulliganCards = new List<CardVisual>();
        private readonly HashSet<int> _markedCardUids = new HashSet<int>();
        private bool _isMulliganActive;
        private bool _hasCompletedMulligan;
        private float _shieldCascadeElapsed;
        private int _shieldsDealtSteps;

        public bool IsMulliganActive => _isMulliganActive;

        public void CancelOpeningMulligan()
        {
            foreach (CardVisual card in _mulliganCards)
                if (card != null) card.SetMulliganMarked(false);
            _mulliganCards.Clear();
            _markedCardUids.Clear();
            _isMulliganActive = false;
            _hasCompletedMulligan = false;
            if (mulliganOverlayRoot != null) mulliganOverlayRoot.gameObject.SetActive(false);
        }
        public bool HasCompletedMulligan => _hasCompletedMulligan;
        public IReadOnlyList<CardVisual> MulliganCards => _mulliganCards;
        public IReadOnlyCollection<int> MarkedCardUids => _markedCardUids;
        public Transform ConfirmButtonTransform => confirmButtonTransform;
        public float MulliganScaleMultiplier => mulliganScaleMultiplier;
        public float ShieldDealCascadeDelay => shieldDealCascadeDelay;
        public int ShieldsDealtSteps => _shieldsDealtSteps;

        public event Action<IReadOnlyCollection<int>> OnMulliganConfirmed;

        private void Awake()
        {
            EnsureBuilt();
        }

        public void EnsureBuilt()
        {
            if (choreographer == null)
            {
                choreographer = GetComponent<BattlefieldChoreographer>();
                if (choreographer == null) choreographer = FindAnyObjectByType<BattlefieldChoreographer>();
            }
            if (eventQueue == null)
            {
                eventQueue = GetComponent<VisualEventQueue>();
                if (eventQueue == null) eventQueue = FindAnyObjectByType<VisualEventQueue>();
            }
            if (interactionController == null)
            {
                interactionController = FindAnyObjectByType<CardInteractionController>();
            }
            if (audioAndProps == null)
            {
                audioAndProps = FindAnyObjectByType<TabletopAudioAndProps>();
            }

            GameObject stage = GameObject.Find("EnchantedTableStage");
            Transform parent = stage != null ? stage.transform : transform;

            if (mulliganOverlayRoot == null)
            {
                Transform existing = parent.Find("OpeningMulliganOverlay");
                if (existing != null)
                {
                    mulliganOverlayRoot = existing;
                    confirmButtonTransform = existing.Find("ConfirmMulliganButton3D");
                    if (confirmButtonTransform != null)
                    {
                        confirmButtonRenderer = confirmButtonTransform.GetComponent<Renderer>();
                        confirmButtonSpring = confirmButtonTransform.GetComponent<SecondOrderSpring>();
                        confirmButtonLabel = confirmButtonTransform.GetComponentInChildren<TMP_Text>();
                    }
                    Transform headerTr = existing.Find("MulliganHeaderLabel");
                    if (headerTr != null) mulliganHeaderLabel = headerTr.GetComponent<TMP_Text>();
                }
                else
                {
                    GameObject rootGo = new GameObject("OpeningMulliganOverlay");
                    rootGo.transform.SetParent(parent, false);
                    rootGo.transform.localPosition = Vector3.zero;
                    mulliganOverlayRoot = rootGo.transform;

                    mulliganHeaderLabel = CreateWorldLabel(
                        mulliganOverlayRoot,
                        "MulliganHeaderLabel",
                        "OPENING HAND — CLICK CARDS TO REPLACE",
                        new Vector3(0f, 4.15f, -1.25f),
                        new Vector2(10.5f, 1.1f),
                        2.65f,
                        new Color(1.0f, 0.92f, 0.56f, 1f));

                    GameObject btnGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
                    btnGo.name = "ConfirmMulliganButton3D";
                    btnGo.transform.SetParent(mulliganOverlayRoot, false);
                    Vector3 btnPos = new Vector3(0f, 3.65f, -5.05f);
                    Vector3 btnScale = new Vector3(2.85f, 0.30f, 0.92f);
                    btnGo.transform.localPosition = btnPos;
                    btnGo.transform.localScale = btnScale;

                    confirmButtonTransform = btnGo.transform;
                    confirmButtonRenderer = btnGo.GetComponent<Renderer>();
                    if (confirmButtonRenderer != null)
                    {
                        Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
                        if (borderMat != null) confirmButtonRenderer.sharedMaterial = borderMat;
                        MaterialPropertyBlock block = new MaterialPropertyBlock();
                        confirmButtonRenderer.GetPropertyBlock(block);
                        Color btnCol = new Color(0.20f, 0.68f, 0.44f, 1f);
                        block.SetColor("_BaseColor", btnCol);
                        block.SetColor("_Color", btnCol);
                        block.SetColor("_GlowColor", new Color(0.204f, 0.827f, 0.600f, 1f));
                        block.SetColor("_GlowSecondaryColor", new Color(0.984f, 0.749f, 0.141f, 1f));
                        block.SetFloat("_GlowIntensity", 0.9f);
                        block.SetFloat("_HighlightState", 1f);
                        confirmButtonRenderer.SetPropertyBlock(block);
                    }

                    confirmButtonSpring = btnGo.AddComponent<SecondOrderSpring>();
                    confirmButtonSpring.SetTarget(btnPos, Quaternion.Euler(28f, 0f, 0f), btnScale, true);

                    confirmButtonLabel = CreateWorldLabel(
                        btnGo.transform,
                        "ConfirmButtonLabel",
                        "CONFIRM HAND",
                        new Vector3(0f, 0.58f, 0f),
                        new Vector2(2.6f, 0.65f),
                        1.75f,
                        Color.white);

                    mulliganOverlayRoot.gameObject.SetActive(false);
                }
            }
        }

        private static TMP_Text CreateWorldLabel(
            Transform parent,
            string name,
            string text,
            Vector3 localPos,
            Vector2 rectSize,
            float fontSize,
            Color color)
        {
            GameObject go = new GameObject(name);
            go.transform.SetParent(parent, false);
            go.transform.localPosition = localPos;
            go.transform.localRotation = Quaternion.Euler(56f, 0f, 0f);
            TextMeshPro tmp = go.AddComponent<TextMeshPro>();
            RectTransform rt = go.GetComponent<RectTransform>();
            if (rt != null) rt.sizeDelta = rectSize;
            tmp.text = text;
            tmp.fontSize = fontSize;
            tmp.color = color;
            tmp.alignment = TextAlignmentOptions.Center;
            tmp.sortingOrder = 25;
            return tmp;
        }

        /// <summary>
        /// Begins the Opening Mulligan Phase by fanning Alice's initial 5 drawn cards across the
        /// center of the camera viewport at 1.6x scale and pausing regular drag interaction until confirmed.
        /// </summary>
        public void BeginOpeningMulligan(bool snapImmediately = false)
        {
            EnsureBuilt();
            if (choreographer == null || choreographer.AliceHandAnchor == null)
            {
                return;
            }

            _isMulliganActive = true;
            _hasCompletedMulligan = false;
            _markedCardUids.Clear();
            _mulliganCards.Clear();

            if (mulliganOverlayRoot != null)
            {
                mulliganOverlayRoot.gameObject.SetActive(true);
            }
            if (interactionController != null)
            {
                interactionController.InteractionEnabled = false;
            }

            IReadOnlyList<CardVisual> handCards = choreographer.AliceHandAnchor.Cards;
            for (int i = 0; i < handCards.Count; i++)
            {
                CardVisual cv = handCards[i];
                if (cv != null)
                {
                    cv.SetMulliganMarked(false);
                    _mulliganCards.Add(cv);
                }
            }

            LayoutMulliganCardsInViewport(snapImmediately);
        }

        public void LayoutMulliganCardsInViewport(bool snapImmediately = false)
        {
            if (!_isMulliganActive || choreographer == null || choreographer.AliceHandAnchor == null)
            {
                return;
            }

            Transform handTransform = choreographer.AliceHandAnchor.transform;
            int count = _mulliganCards.Count;
            if (count == 0)
            {
                return;
            }

            float totalWidth = (count - 1) * mulliganCardSpacing;
            float startX = -totalWidth * 0.5f;
            Vector3 targetScale = Vector3.one * mulliganScaleMultiplier;

            for (int i = 0; i < count; i++)
            {
                CardVisual card = _mulliganCards[i];
                if (card == null)
                {
                    continue;
                }

                Vector3 worldPos = mulliganCenterWorld + new Vector3(startX + i * mulliganCardSpacing, i * 0.015f, 0f);
                Vector3 localPos = handTransform.InverseTransformPoint(worldPos);
                Quaternion localRot = Quaternion.Euler(24f, 0f, 0f);
                card.Spring.SetTarget(localPos, localRot, targetScale, snapImmediately);
            }
        }

        /// <summary>
        /// Toggles the "Replace / Redraw" badge on a card during the Opening Mulligan phase.
        /// </summary>
        public bool ToggleCardMulliganSelection(CardVisual card)
        {
            if (!_isMulliganActive || card == null || !_mulliganCards.Contains(card))
            {
                return false;
            }

            bool nowMarked;
            if (_markedCardUids.Contains(card.CardUid))
            {
                _markedCardUids.Remove(card.CardUid);
                nowMarked = false;
            }
            else
            {
                _markedCardUids.Add(card.CardUid);
                nowMarked = true;
            }

            card.SetMulliganMarked(nowMarked);
            if (card.Spring != null)
            {
                card.Spring.TriggerImpactSquashPunch();
            }
            if (audioAndProps != null)
            {
                audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.CardDrawSnap);
            }

            if (confirmButtonLabel != null)
            {
                confirmButtonLabel.text = _markedCardUids.Count > 0
                    ? $"REDRAW ({_markedCardUids.Count}) & START"
                    : "CONFIRM HAND";
            }
            return nowMarked;
        }

        /// <summary>
        /// Confirms the opening hand, clears replace badges, springs Alice's 5 cards smoothly down
        /// into AliceHandAnchor at 1.0x scale, and deals the 6 Shields into AliceShieldRow and
        /// WhiteRabbitShieldRow with staggered 0.08s cascade delays.
        /// </summary>
        public void ConfirmMulliganAndDealShields(bool snapImmediately = false)
        {
            EnsureBuilt();
            if (!_isMulliganActive && _hasCompletedMulligan)
            {
                return;
            }

            if (confirmButtonSpring != null)
            {
                confirmButtonSpring.TriggerImpactSquashPunch();
            }
            if (audioAndProps != null)
            {
                audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.ButtonClick);
            }

            if (mulliganHeaderLabel != null)
            {
                mulliganHeaderLabel.text = "OPENING HAND — CLICK CARDS TO REPLACE";
            }

            // If cards were marked for redraw, advance the match seed and re-sync Alice's hand so marked cards are genuinely redrawn
            if (_markedCardUids.Count > 0 && eventQueue != null && eventQueue.IsNativeConnected && choreographer != null)
            {
                eventQueue.ReseedAndReinitializeBridge((ulong)_markedCardUids.Count + 7UL);
                eventQueue.DrainAllEventsSynchronously(0.05f);
                choreographer.SyncBoardStateFromNative(true);

                _mulliganCards.Clear();
                if (choreographer.AliceHandAnchor != null)
                {
                    foreach (CardVisual newCard in choreographer.AliceHandAnchor.Cards)
                    {
                        if (newCard != null)
                        {
                            newCard.SetMulliganMarked(false);
                            newCard.transform.position = new Vector3(6.5f, 0.6f, -3.4f);
                            newCard.transform.rotation = Quaternion.Euler(0f, 0f, 180f);
                            _mulliganCards.Add(newCard);
                        }
                    }
                }
            }

            foreach (CardVisual c in _mulliganCards)
            {
                if (c != null)
                {
                    c.SetMulliganMarked(false);
                }
            }

            _isMulliganActive = false;
            _hasCompletedMulligan = true;

            if (mulliganOverlayRoot != null)
            {
                mulliganOverlayRoot.gameObject.SetActive(false);
            }

            // Spring Alice's hand cards smoothly down into AliceHandAnchor
            if (choreographer != null && choreographer.AliceHandAnchor != null)
            {
                choreographer.AliceHandAnchor.RebuildLayout(snapImmediately);
            }

            // Deal the 6 Shields into AliceShieldRow and WhiteRabbitShieldRow with staggered 0.08s cascade delays
            ExecuteShieldDealCascade(snapImmediately);

            if (interactionController != null)
            {
                interactionController.InteractionEnabled = true;
            }

            OnMulliganConfirmed?.Invoke(_markedCardUids);
        }

        public void ExecuteShieldDealCascade(bool snapImmediately = false)
        {
            if (choreographer == null)
            {
                return;
            }

            _shieldsDealtSteps = 0;
            BoardRowLayout aliceShields = choreographer.AliceShieldRow;
            BoardRowLayout rabbitShields = choreographer.WhiteRabbitShieldRow;

            Vector3 aliceDeckOrigin = new Vector3(6.5f, 0.8f, -3.4f);
            Vector3 rabbitDeckOrigin = new Vector3(6.5f, 0.8f, 3.4f);

            int maxCount = Mathf.Max(
                aliceShields != null ? aliceShields.Cards.Count : 0,
                rabbitShields != null ? rabbitShields.Cards.Count : 0);

            for (int i = 0; i < maxCount; i++)
            {
                float delayFactor = i * shieldDealCascadeDelay;
                if (aliceShields != null && i < aliceShields.Cards.Count)
                {
                    CardVisual sCard = aliceShields.Cards[i];
                    if (sCard != null && !snapImmediately)
                    {
                        sCard.transform.position = aliceDeckOrigin + Vector3.up * (i * 0.04f);
                        sCard.Spring.AddImpulse(new Vector3(-3.5f, 1.5f + delayFactor * 2f, 0f));
                    }
                    _shieldsDealtSteps++;
                }

                if (rabbitShields != null && i < rabbitShields.Cards.Count)
                {
                    CardVisual sCard = rabbitShields.Cards[i];
                    if (sCard != null && !snapImmediately)
                    {
                        sCard.transform.position = rabbitDeckOrigin + Vector3.up * (i * 0.04f);
                        sCard.Spring.AddImpulse(new Vector3(-3.5f, 1.5f + delayFactor * 2f, 0f));
                    }
                    _shieldsDealtSteps++;
                }
            }

            if (aliceShields != null) aliceShields.RebuildLayout(snapImmediately);
            if (rabbitShields != null) rabbitShields.RebuildLayout(snapImmediately);
        }

        private void Update()
        {
            var router = FindAnyObjectByType<Heav3ns.UI.Navigation.HubNavigationRouter>();
            if (router != null && (router.IsTransitioning || router.IsPauseOpen)) return;
            if (!_isMulliganActive)
            {
                return;
            }

            if (UnityEngine.Input.GetMouseButtonDown(0))
            {
                Camera cam = Camera.main;
                if (cam != null)
                {
                    Ray ray = cam.ScreenPointToRay(UnityEngine.Input.mousePosition);
                    ProcessMulliganClickRay(ray);
                }
            }
        }

        public bool ProcessMulliganClickRay(Ray ray)
        {
            if (!_isMulliganActive)
            {
                return false;
            }

            RaycastHit[] hits = Physics.RaycastAll(ray, 100f);
            for (int i = 0; i < hits.Length; i++)
            {
                Transform hitTr = hits[i].transform;
                if (confirmButtonTransform != null && (hitTr == confirmButtonTransform || hitTr.IsChildOf(confirmButtonTransform)))
                {
                    ConfirmMulliganAndDealShields(false);
                    return true;
                }

                CardVisual cv = hitTr.GetComponentInParent<CardVisual>();
                if (cv != null && _mulliganCards.Contains(cv))
                {
                    ToggleCardMulliganSelection(cv);
                    return true;
                }
            }
            return false;
        }
    }
}

