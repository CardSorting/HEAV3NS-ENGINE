using System;
using Heav3ns.UnityBridge;
using Heav3ns.Visuals;
using TMPro;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>
    /// Diegetic 3D Tabletop HUD on EnchantedTableStage/BoardSurface.
    /// Provides:
    /// 1. 3D clickable End Turn Lever/Button with SecondOrderSpring depression and state coloring
    ///    (playable actions remaining vs. turn complete).
    /// 2. World-space UI labels for Mana/Resource crystals, Deck counts, Graveyard/Discard piles,
    ///    and Hero/Player life/shield totals for both Alice and White Rabbit.
    /// 3. Match Reset / New Game 3D clickable control when GameOver is reached.
    /// 4. Center-screen Turn / GameOver Banner.
    /// </summary>
    [DisallowMultipleComponent]
    public class DiegeticTabletopHud : MonoBehaviour
    {
        [Header("End Turn 3D Lever / Button")]
        [SerializeField] private Transform endTurnButtonTransform;
        [SerializeField] private Renderer endTurnButtonRenderer;
        [SerializeField] private SecondOrderSpring endTurnButtonSpring;
        [SerializeField] private TMP_Text endTurnButtonLabel;

        [Header("Match Reset / New Game Control")]
        [SerializeField] private Transform matchResetButtonTransform;
        [SerializeField] private Renderer matchResetButtonRenderer;
        [SerializeField] private SecondOrderSpring matchResetButtonSpring;
        [SerializeField] private TMP_Text matchResetButtonLabel;

        [Header("Center Banner")]
        [SerializeField] private Transform centerBannerTransform;
        [SerializeField] private TMP_Text centerBannerTitle;
        [SerializeField] private TMP_Text centerBannerSubtitle;

        [Header("Alice (Seat 0) World-Space Labels")]
        [SerializeField] private TMP_Text aliceManaLabel;
        [SerializeField] private TMP_Text aliceDeckLabel;
        private string _playerName = "ALICE";
        private string _opponentName = "RABBIT";
        [SerializeField] private TMP_Text aliceDiscardLabel;
        [SerializeField] private TMP_Text aliceLifeShieldsLabel;

        [Header("White Rabbit (Seat 1) World-Space Labels")]
        [SerializeField] private TMP_Text whiteRabbitManaLabel;
        [SerializeField] private TMP_Text whiteRabbitDeckLabel;
        [SerializeField] private TMP_Text whiteRabbitDiscardLabel;
        [SerializeField] private TMP_Text whiteRabbitLifeShieldsLabel;

        [Header("References")]
        [SerializeField] private VisualEventQueue eventQueue;
        [SerializeField] private TabletopAudioAndProps audioAndProps;

        private Vector3 _endTurnRestPos = new Vector3(7.2f, 0.28f, 0f);
        private Vector3 _matchResetRestPos = new Vector3(7.2f, 0.28f, -2.0f);
        private float _bannerTimer;
        private bool _isGameOver;
        private int _activeSeat;
        private int _aliceActions = 6;
        private MaterialPropertyBlock _propBlock;

        public Transform EndTurnButtonTransform => endTurnButtonTransform;
        public Transform MatchResetButtonTransform => matchResetButtonTransform;
        public TMP_Text AliceManaLabel => aliceManaLabel;
        public TMP_Text WhiteRabbitManaLabel => whiteRabbitManaLabel;
        public bool IsGameOver => _isGameOver;

        public event Action OnEndTurnRequested;
        public event Action OnMatchResetRequested;

        private void Awake()
        {
            EnsureHudBuilt();
        }

        public void EnsureHudBuilt()
        {
            if (eventQueue == null)
            {
                eventQueue = FindAnyObjectByType<VisualEventQueue>();
            }
            if (audioAndProps == null)
            {
                audioAndProps = FindAnyObjectByType<TabletopAudioAndProps>();
            }

            GameObject stage = GameObject.Find("EnchantedTableStage");
            Transform parent = stage != null ? stage.transform : transform;

            // 1. 3D End Turn Button on the right edge of BoardSurface
            if (endTurnButtonTransform == null)
            {
                Transform existing = parent.Find("EndTurnButton3D");
                if (existing != null)
                {
                    endTurnButtonTransform = existing;
                    endTurnButtonRenderer = existing.GetComponent<Renderer>();
                    endTurnButtonSpring = existing.GetComponent<SecondOrderSpring>();
                    endTurnButtonLabel = existing.GetComponentInChildren<TMP_Text>();
                }
                else
                {
                    endTurnButtonTransform = Create3DButton(
                        parent,
                        "EndTurnButton3D",
                        _endTurnRestPos,
                        new Vector3(1.85f, 0.28f, 0.95f),
                        new Color(0.24f, 0.68f, 0.42f, 1f),
                        "END TURN",
                        out endTurnButtonRenderer,
                        out endTurnButtonSpring,
                        out endTurnButtonLabel);
                }
            }

            // 2. 3D Match Reset / New Game Control
            if (matchResetButtonTransform == null)
            {
                Transform existingReset = parent.Find("MatchResetButton3D");
                if (existingReset != null)
                {
                    matchResetButtonTransform = existingReset;
                    matchResetButtonRenderer = existingReset.GetComponent<Renderer>();
                    matchResetButtonSpring = existingReset.GetComponent<SecondOrderSpring>();
                    matchResetButtonLabel = existingReset.GetComponentInChildren<TMP_Text>();
                }
                else
                {
                    matchResetButtonTransform = Create3DButton(
                        parent,
                        "MatchResetButton3D",
                        _matchResetRestPos,
                        new Vector3(1.85f, 0.26f, 0.85f),
                        new Color(0.84f, 0.56f, 0.20f, 1f),
                        "NEW GAME",
                        out matchResetButtonRenderer,
                        out matchResetButtonSpring,
                        out matchResetButtonLabel);
                }
            }

            // 3. Center Banner
            if (centerBannerTransform == null)
            {
                Transform existingBanner = parent.Find("CenterTurnBanner");
                if (existingBanner != null)
                {
                    centerBannerTransform = existingBanner;
                    Transform t1 = existingBanner.Find("BannerTitle");
                    Transform t2 = existingBanner.Find("BannerSubtitle");
                    if (t1 != null) centerBannerTitle = t1.GetComponent<TMP_Text>();
                    if (t2 != null) centerBannerSubtitle = t2.GetComponent<TMP_Text>();
                }
                else
                {
                    GameObject bannerGo = new GameObject("CenterTurnBanner");
                    bannerGo.transform.SetParent(parent, false);
                    bannerGo.transform.localPosition = new Vector3(0f, 1.85f, 0f);
                    centerBannerTransform = bannerGo.transform;

                    centerBannerTitle = CreateWorldLabel(
                        bannerGo.transform,
                        "BannerTitle",
                        "TURN 1",
                        new Vector3(0f, 0.2f, 0.25f),
                        new Vector2(8f, 1.2f),
                        4.5f,
                        new Color(1.0f, 0.92f, 0.55f, 1f));

                    centerBannerSubtitle = CreateWorldLabel(
                        bannerGo.transform,
                        "BannerSubtitle",
                        "Alice's Turn",
                        new Vector3(0f, 0.2f, -0.35f),
                        new Vector2(8f, 0.9f),
                        2.8f,
                        Color.white);

                    bannerGo.SetActive(false);
                }
            }

            // 4. Alice & White Rabbit Diegetic Resource & Pile Readouts
            Transform hudGroup = parent.Find("DiegeticCountersGroup");
            if (hudGroup == null)
            {
                GameObject groupGo = new GameObject("DiegeticCountersGroup");
                groupGo.transform.SetParent(parent, false);
                hudGroup = groupGo.transform;
            }

            if (aliceManaLabel == null)
            {
                aliceManaLabel = FindOrCreateCounter(hudGroup, "AliceManaCounter", "ALICE MANA: 6/6", new Vector3(-7.0f, 0.18f, -4.1f), new Color(0.42f, 0.88f, 1.0f, 1f));
            }
            if (aliceLifeShieldsLabel == null)
            {
                aliceLifeShieldsLabel = FindOrCreateCounter(hudGroup, "AliceLifeShieldsCounter", "ALICE SHIELDS: 6", new Vector3(-7.0f, 0.18f, -3.3f), new Color(0.98f, 0.84f, 0.38f, 1f));
            }
            if (aliceDeckLabel == null)
            {
                aliceDeckLabel = FindOrCreateCounter(hudGroup, "AliceDeckCounter", "ALICE DECK: 29", new Vector3(6.8f, 0.18f, -3.5f), new Color(0.86f, 0.86f, 0.94f, 1f));
            }
            if (aliceDiscardLabel == null)
            {
                aliceDiscardLabel = FindOrCreateCounter(hudGroup, "AliceDiscardCounter", "ALICE GRAVEYARD: 0", new Vector3(6.8f, 0.18f, -4.3f), new Color(0.75f, 0.65f, 0.82f, 1f));
            }

            if (whiteRabbitManaLabel == null)
            {
                whiteRabbitManaLabel = FindOrCreateCounter(hudGroup, "WhiteRabbitManaCounter", "RABBIT MANA: 6/6", new Vector3(-7.0f, 0.18f, 4.1f), new Color(1.0f, 0.56f, 0.48f, 1f));
            }
            if (whiteRabbitLifeShieldsLabel == null)
            {
                whiteRabbitLifeShieldsLabel = FindOrCreateCounter(hudGroup, "WhiteRabbitLifeShieldsCounter", "RABBIT SHIELDS: 6", new Vector3(-7.0f, 0.18f, 3.3f), new Color(0.98f, 0.84f, 0.38f, 1f));
            }
            if (whiteRabbitDeckLabel == null)
            {
                whiteRabbitDeckLabel = FindOrCreateCounter(hudGroup, "WhiteRabbitDeckCounter", "RABBIT DECK: 29", new Vector3(6.8f, 0.18f, 3.5f), new Color(0.86f, 0.86f, 0.94f, 1f));
            }
            if (whiteRabbitDiscardLabel == null)
            {
                whiteRabbitDiscardLabel = FindOrCreateCounter(hudGroup, "WhiteRabbitDiscardCounter", "RABBIT GRAVEYARD: 0", new Vector3(6.8f, 0.18f, 4.3f), new Color(0.75f, 0.65f, 0.82f, 1f));
            }

            UpdateEndTurnButtonState(_activeSeat == 0, _aliceActions > 0);
        }

        private static TMP_Text FindOrCreateCounter(Transform parent, string name, string defaultText, Vector3 localPos, Color color)
        {
            Transform existing = parent.Find(name);
            if (existing != null)
            {
                TMP_Text t = existing.GetComponent<TMP_Text>();
                if (t != null) return t;
            }
            return CreateWorldLabel(parent, name, defaultText, localPos, new Vector2(4.2f, 0.55f), 1.9f, color);
        }

        private static Transform Create3DButton(
            Transform parent,
            string name,
            Vector3 localPos,
            Vector3 scale,
            Color baseColor,
            string labelText,
            out Renderer rend,
            out SecondOrderSpring spring,
            out TMP_Text label)
        {
            GameObject btnGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
            btnGo.name = name;
            btnGo.transform.SetParent(parent, false);
            btnGo.transform.localPosition = localPos;
            btnGo.transform.localScale = scale;

            rend = btnGo.GetComponent<Renderer>();
            if (rend != null)
            {
                MaterialPropertyBlock block = new MaterialPropertyBlock();
                rend.GetPropertyBlock(block);
                block.SetColor("_BaseColor", baseColor);
                block.SetColor("_Color", baseColor);
                rend.SetPropertyBlock(block);
            }

            spring = btnGo.AddComponent<SecondOrderSpring>();
            spring.SetTarget(localPos, Quaternion.identity, scale, true);

            label = CreateWorldLabel(
                btnGo.transform,
                "ButtonLabel",
                labelText,
                new Vector3(0f, 0.56f, 0f),
                new Vector2(1.8f, 0.6f),
                1.65f,
                Color.white);

            return btnGo.transform;
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
            go.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);

            TextMeshPro tmp = go.AddComponent<TextMeshPro>();
            RectTransform rt = go.GetComponent<RectTransform>();
            if (rt != null)
            {
                rt.sizeDelta = rectSize;
            }
            tmp.text = text;
            tmp.fontSize = fontSize;
            tmp.color = color;
            tmp.alignment = TextAlignmentOptions.Center;
            return tmp;
        }

        public void SetPlayerNames(string player, string opponent)
        {
            _playerName = string.IsNullOrEmpty(player) ? "PLAYER" : player.ToUpperInvariant();
            _opponentName = string.IsNullOrEmpty(opponent) ? "OPPONENT" : opponent.ToUpperInvariant();
        }

        public void UpdatePlayerCounters(
            int activeSeat,
            int aliceMana,
            int aliceShields,
            int aliceDeck,
            int aliceDiscard,
            int rabbitMana,
            int rabbitShields,
            int rabbitDeck,
            int rabbitDiscard,
            bool hasPlayableActions,
            bool isGameOver)
        {
            EnsureHudBuilt();
            _activeSeat = activeSeat;
            _aliceActions = aliceMana;
            _isGameOver = isGameOver;

            if (aliceManaLabel != null) aliceManaLabel.text = $"{_playerName} MANA: {aliceMana}/6";
            if (aliceLifeShieldsLabel != null) aliceLifeShieldsLabel.text = $"{_playerName} SHIELDS: {aliceShields}";
            if (aliceDeckLabel != null) aliceDeckLabel.text = $"{_playerName} DECK: {aliceDeck}";
            if (aliceDiscardLabel != null) aliceDiscardLabel.text = $"{_playerName} GRAVEYARD: {aliceDiscard}";

            if (whiteRabbitManaLabel != null) whiteRabbitManaLabel.text = $"{_opponentName} MANA: {rabbitMana}/6";
            if (whiteRabbitLifeShieldsLabel != null) whiteRabbitLifeShieldsLabel.text = $"{_opponentName} SHIELDS: {rabbitShields}";
            if (whiteRabbitDeckLabel != null) whiteRabbitDeckLabel.text = $"{_opponentName} DECK: {rabbitDeck}";
            if (whiteRabbitDiscardLabel != null) whiteRabbitDiscardLabel.text = $"{_opponentName} GRAVEYARD: {rabbitDiscard}";

            UpdateEndTurnButtonState(activeSeat == 0 && !isGameOver, hasPlayableActions);
            if (matchResetButtonTransform != null)
            {
                matchResetButtonTransform.gameObject.SetActive(isGameOver);
            }
        }

        public void UpdateEndTurnButtonState(bool isAliceTurn, bool playableActionsRemaining)
        {
            if (endTurnButtonRenderer == null)
            {
                return;
            }
            if (_propBlock == null)
            {
                _propBlock = new MaterialPropertyBlock();
            }

            // State coloring: warm amber/emerald when playable actions remain vs. bright glowing gold/cyan when turn complete
            Color stateColor;
            if (!isAliceTurn)
            {
                stateColor = new Color(0.32f, 0.32f, 0.38f, 1f);
                if (endTurnButtonLabel != null) endTurnButtonLabel.text = "OPPONENT TURN";
            }
            else if (playableActionsRemaining)
            {
                stateColor = new Color(0.22f, 0.65f, 0.44f, 1f);
                if (endTurnButtonLabel != null) endTurnButtonLabel.text = "END TURN";
            }
            else
            {
                stateColor = new Color(0.95f, 0.78f, 0.22f, 1f);
                if (endTurnButtonLabel != null) endTurnButtonLabel.text = "TURN COMPLETE";
            }

            endTurnButtonRenderer.GetPropertyBlock(_propBlock);
            _propBlock.SetColor("_BaseColor", stateColor);
            _propBlock.SetColor("_Color", stateColor);
            endTurnButtonRenderer.SetPropertyBlock(_propBlock);
        }

        public void ShowBanner(string title, string subtitle, float duration = 0.6f)
        {
            EnsureHudBuilt();
            if (centerBannerTransform == null)
            {
                return;
            }
            centerBannerTransform.gameObject.SetActive(true);
            if (centerBannerTitle != null) centerBannerTitle.text = title;
            if (centerBannerSubtitle != null) centerBannerSubtitle.text = subtitle;
            _bannerTimer = Mathf.Max(0.2f, duration);
        }

        private void Update()
        {
            if (_bannerTimer > 0f && !_isGameOver)
            {
                _bannerTimer -= Time.deltaTime;
                if (_bannerTimer <= 0f && centerBannerTransform != null)
                {
                    centerBannerTransform.gameObject.SetActive(false);
                }
            }

            var router = FindAnyObjectByType<Heav3ns.UI.Navigation.HubNavigationRouter>();
            if (router != null && (router.Current != Heav3ns.UI.Navigation.HubScreenState.ActiveBattlefield || router.IsTransitioning || router.IsPauseOpen)) return;
            if (UnityEngine.Input.GetMouseButtonDown(0))
            {
                Camera cam = Camera.main;
                if (cam != null)
                {
                    Ray ray = cam.ScreenPointToRay(UnityEngine.Input.mousePosition);
                    if (Physics.Raycast(ray, out RaycastHit hit, 100f))
                    {
                        TryClickHudControl(hit.transform);
                    }
                }
            }
        }

        public bool TryClickHudControl(Transform hitTransform)
        {
            EnsureHudBuilt();
            if (hitTransform == null)
            {
                return false;
            }

            if (endTurnButtonTransform != null &&
                (hitTransform == endTurnButtonTransform || hitTransform.IsChildOf(endTurnButtonTransform)))
            {
                PressEndTurnLever();
                return true;
            }

            if (matchResetButtonTransform != null &&
                (hitTransform == matchResetButtonTransform || hitTransform.IsChildOf(matchResetButtonTransform)))
            {
                PressMatchResetControl();
                return true;
            }

            return false;
        }

        public void PressEndTurnLever()
        {
            EnsureHudBuilt();
            if (endTurnButtonSpring != null)
            {
                endTurnButtonSpring.AddImpulse(new Vector3(0f, -4.5f, 0f));
                endTurnButtonSpring.TriggerImpactSquashPunch();
            }
            if (audioAndProps != null)
            {
                audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.ButtonClick);
            }
            OnEndTurnRequested?.Invoke();
        }

        public void SetGameOverState(bool gameOver)
        {
            EnsureHudBuilt();
            _isGameOver = gameOver;
            if (matchResetButtonTransform != null)
            {
                matchResetButtonTransform.gameObject.SetActive(gameOver);
            }
            UpdateEndTurnButtonState(_activeSeat == 0 && !gameOver, _aliceActions > 0);
        }

        public void PressMatchResetControl()
        {
            EnsureHudBuilt();
            if (matchResetButtonSpring != null)
            {
                matchResetButtonSpring.AddImpulse(new Vector3(0f, -4.5f, 0f));
                matchResetButtonSpring.TriggerImpactSquashPunch();
            }
            if (audioAndProps != null)
            {
                audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.ButtonClick);
            }
            _isGameOver = false;
            if (matchResetButtonTransform != null)
            {
                matchResetButtonTransform.gameObject.SetActive(false);
            }
            if (centerBannerTransform != null)
            {
                centerBannerTransform.gameObject.SetActive(false);
            }
            OnMatchResetRequested?.Invoke();
        }
    }
}
