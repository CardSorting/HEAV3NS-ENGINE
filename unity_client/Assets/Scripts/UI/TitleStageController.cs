using System;
using Heav3ns.Choreography;
using Heav3ns.Input;
using Heav3ns.Visuals;
using TMPro;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>
    /// Phase 4 Diegetic Title Screen & Match Launcher (TitleStageController.cs).
    /// Keeps the camera inside the physical room (Battlefield.unity), starting zoomed into an ornate
    /// title plaque / closed tabletop box above EnchantedTableStage with 3D clickable buttons:
    /// - Play Match (Alice vs. White Rabbit)
    /// - Practice / AI Speed Toggle
    /// - Quit
    /// Clicking Play Match smoothly transitions the camera (SecondOrderSpring) to the active 2.5D gameplay
    /// viewpoint and triggers the Opening Mulligan sequence.
    /// Incorporates SkipIntroInSimulations so automated headless tests bypass the title menu when needed.
    /// </summary>
    [DisallowMultipleComponent]
    public class TitleStageController : MonoBehaviour
    {
        [Header("Simulation & Startup Flags")]
        [SerializeField] private bool skipIntroInSimulations = false;

        [Header("Camera Viewpoint Targets")]
        [SerializeField] private Vector3 titleCameraPosition = new Vector3(0f, 6.2f, -3.6f);
        [SerializeField] private Vector3 titleCameraEuler = new Vector3(44f, 0f, 0f);
        [SerializeField] private Vector3 gameplayCameraPosition = new Vector3(0f, 11.5f, -7.8f);
        [SerializeField] private Vector3 gameplayCameraEuler = new Vector3(56f, 0f, 0f);

        [Header("Diegetic Diorama Hierarchy")]
        [SerializeField] private Transform dioramaRoot;
        [SerializeField] private Transform playMatchButtonTransform;
        [SerializeField] private SecondOrderSpring playMatchButtonSpring;
        [SerializeField] private Transform aiSpeedButtonTransform;
        [SerializeField] private SecondOrderSpring aiSpeedButtonSpring;
        [SerializeField] private TMP_Text aiSpeedButtonLabel;
        [SerializeField] private Transform quitButtonTransform;
        [SerializeField] private SecondOrderSpring quitButtonSpring;

        [Header("Scene References")]
        [SerializeField] private Camera mainCamera;
        [SerializeField] private SecondOrderSpring cameraSpring;
        [SerializeField] private MulliganController mulliganController;
        [SerializeField] private CardInteractionController interactionController;
        [SerializeField] private TabletopAudioAndProps audioAndProps;

        private bool _isTitleActive;
        private bool _fastAiSpeed;

        public bool SkipIntroInSimulations
        {
            get => skipIntroInSimulations;
            set => skipIntroInSimulations = value;
        }

        public bool IsTitleActive => _isTitleActive && dioramaRoot != null && dioramaRoot.gameObject.activeSelf;
        public bool FastAiSpeed => _fastAiSpeed;
        public Transform DioramaRoot => dioramaRoot;
        public Transform PlayMatchButtonTransform => playMatchButtonTransform;
        public Transform AiSpeedButtonTransform => aiSpeedButtonTransform;
        public Transform QuitButtonTransform => quitButtonTransform;
        public SecondOrderSpring CameraSpring => cameraSpring;

        public event Action OnPlayMatchLaunched;

        private void Awake()
        {
            EnsureBuilt();
        }

        private void Start()
        {
            EnsureBuilt();
            if (!Application.isBatchMode && !skipIntroInSimulations)
            {
                GachaHubLobbyController hub = GetComponent<GachaHubLobbyController>();
                if (hub == null) hub = gameObject.AddComponent<GachaHubLobbyController>();
                hub.Initialize(this);
            }
            else
            {
                HideTitleDioramaToGameplay(true, false);
            }
        }

        public void EnsureBuilt()
        {
            if (mainCamera == null)
            {
                mainCamera = Camera.main != null ? Camera.main : FindAnyObjectByType<Camera>();
            }
            if (mainCamera != null && cameraSpring == null)
            {
                cameraSpring = mainCamera.GetComponent<SecondOrderSpring>();
                if (cameraSpring == null)
                {
                    cameraSpring = mainCamera.gameObject.AddComponent<SecondOrderSpring>();
                }
            }
            if (mulliganController == null)
            {
                mulliganController = FindAnyObjectByType<MulliganController>();
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

            if (dioramaRoot == null)
            {
                Transform existing = parent.Find("TitleScreenDiorama");
                if (existing != null)
                {
                    dioramaRoot = existing;
                    playMatchButtonTransform = existing.Find("PlayMatchButton3D");
                    if (playMatchButtonTransform != null) playMatchButtonSpring = playMatchButtonTransform.GetComponent<SecondOrderSpring>();
                    aiSpeedButtonTransform = existing.Find("AiSpeedToggleButton3D");
                    if (aiSpeedButtonTransform != null)
                    {
                        aiSpeedButtonSpring = aiSpeedButtonTransform.GetComponent<SecondOrderSpring>();
                        aiSpeedButtonLabel = aiSpeedButtonTransform.GetComponentInChildren<TMP_Text>(true);
                    }
                    quitButtonTransform = existing.Find("QuitGameButton3D");
                    if (quitButtonTransform != null) quitButtonSpring = quitButtonTransform.GetComponent<SecondOrderSpring>();
                }
                else
                {
                    GameObject rootGo = new GameObject("TitleScreenDiorama");
                    rootGo.transform.SetParent(parent, false);
                    rootGo.transform.localPosition = new Vector3(0f, 1.45f, -0.45f);
                    dioramaRoot = rootGo.transform;

                    // Ornate Tabletop Box / Plaque
                    GameObject plaqueGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
                    plaqueGo.name = "OrnateTabletopBoxPlaque";
                    Collider plaqueCol = plaqueGo.GetComponent<Collider>();
                    if (plaqueCol != null)
                    {
                        if (Application.isPlaying) Destroy(plaqueCol);
                        else DestroyImmediate(plaqueCol);
                    }
                    plaqueGo.transform.SetParent(dioramaRoot, false);
                    plaqueGo.transform.localPosition = Vector3.zero;
                    plaqueGo.transform.localScale = new Vector3(5.6f, 0.25f, 3.8f);
                    Renderer plaqueRend = plaqueGo.GetComponent<Renderer>();
                    if (plaqueRend != null)
                    {
                        Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
                        if (borderMat != null) plaqueRend.sharedMaterial = borderMat;
                        MaterialPropertyBlock block = new MaterialPropertyBlock();
                        plaqueRend.GetPropertyBlock(block);
                        Color boxCol = new Color(0.18f, 0.14f, 0.26f, 1f);
                        block.SetColor("_BaseColor", boxCol);
                        block.SetColor("_Color", boxCol);
                        plaqueRend.SetPropertyBlock(block);
                    }

                    CreatePlaqueText(
                        dioramaRoot,
                        "TitleBannerText",
                        "HEAV3NS : WONDERLAND",
                        new Vector3(0f, 0.18f, 1.15f),
                        new Vector2(5.2f, 0.8f),
                        2.55f,
                        new Color(0.98f, 0.88f, 0.45f, 1f));

                    CreatePlaqueText(
                        dioramaRoot,
                        "SubtitleBannerText",
                        "Checkmate the Sky — Enchanted Table Edition",
                        new Vector3(0f, 0.18f, 0.62f),
                        new Vector2(5.0f, 0.5f),
                        1.35f,
                        new Color(0.75f, 0.88f, 0.98f, 1f));

                    // 1. Play Match (Alice vs. White Rabbit) 3D Button
                    playMatchButtonTransform = CreateDioramaButton(
                        dioramaRoot,
                        "PlayMatchButton3D",
                        new Vector3(0f, 0.22f, -0.05f),
                        new Vector3(3.8f, 0.22f, 0.68f),
                        new Color(0.20f, 0.66f, 0.44f, 1f),
                        "PLAY MATCH (ALICE VS. WHITE RABBIT)",
                        out playMatchButtonSpring,
                        out _);

                    // 2. Practice / AI Speed Toggle 3D Button
                    aiSpeedButtonTransform = CreateDioramaButton(
                        dioramaRoot,
                        "AiSpeedToggleButton3D",
                        new Vector3(0f, 0.22f, -0.82f),
                        new Vector3(3.2f, 0.20f, 0.58f),
                        new Color(0.26f, 0.46f, 0.74f, 1f),
                        "AI SPEED: STANDARD (1.0x)",
                        out aiSpeedButtonSpring,
                        out aiSpeedButtonLabel);

                    // 3. Quit 3D Button
                    quitButtonTransform = CreateDioramaButton(
                        dioramaRoot,
                        "QuitGameButton3D",
                        new Vector3(0f, 0.22f, -1.48f),
                        new Vector3(2.2f, 0.18f, 0.52f),
                        new Color(0.64f, 0.24f, 0.28f, 1f),
                        "QUIT TABLE",
                        out quitButtonSpring,
                        out _);

                    dioramaRoot.gameObject.SetActive(false);
                }
            }
            if (Application.isPlaying)
            {
                var board = parent.Find("BoardSurface");
                if (board != null && board.TryGetComponent(out Renderer boardRenderer))
                    boardRenderer.sharedMaterial = CardArtRegistry.GetSharedTabletopMaterial();
                var plaque = dioramaRoot != null ? dioramaRoot.Find("OrnateTabletopBoxPlaque") : null;
                if (plaque != null && plaque.TryGetComponent(out Renderer plaqueRenderer))
                {
                    var block = new MaterialPropertyBlock(); plaqueRenderer.GetPropertyBlock(block);
                    var color = new Color(.16f, .06f, .09f, 1f);
                    block.SetColor("_BaseColor", color); block.SetColor("_Color", color); plaqueRenderer.SetPropertyBlock(block);
                }
            }
        }

        private static Transform CreateDioramaButton(
            Transform parent,
            string name,
            Vector3 localPos,
            Vector3 scale,
            Color color,
            string text,
            out SecondOrderSpring spring,
            out TMP_Text label)
        {
            GameObject btnGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
            btnGo.name = name;
            btnGo.transform.SetParent(parent, false);
            btnGo.transform.localPosition = localPos;
            btnGo.transform.localScale = scale;

            Renderer rend = btnGo.GetComponent<Renderer>();
            if (rend != null)
            {
                Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
                if (borderMat != null) rend.sharedMaterial = borderMat;
                MaterialPropertyBlock block = new MaterialPropertyBlock();
                rend.GetPropertyBlock(block);
                block.SetColor("_BaseColor", color);
                block.SetColor("_Color", color);
                rend.SetPropertyBlock(block);
            }

            spring = btnGo.AddComponent<SecondOrderSpring>();
            spring.SetTarget(localPos, Quaternion.identity, scale, true);

            label = CreatePlaqueText(
                btnGo.transform,
                "ButtonLabel",
                text,
                new Vector3(0f, 0.58f, 0f),
                new Vector2(scale.x * 0.95f, 0.55f),
                1.35f,
                Color.white);

            return btnGo.transform;
        }

        private static TMP_Text CreatePlaqueText(
            Transform parent,
            string name,
            string text,
            Vector3 localPos,
            Vector2 size,
            float fontSize,
            Color color)
        {
            GameObject go = new GameObject(name);
            go.transform.SetParent(parent, false);
            go.transform.localPosition = localPos;
            go.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            TextMeshPro tmp = go.AddComponent<TextMeshPro>();
            RectTransform rt = go.GetComponent<RectTransform>();
            if (rt != null) rt.sizeDelta = size;
            tmp.text = text;
            tmp.fontSize = fontSize;
            tmp.color = color;
            tmp.alignment = TextAlignmentOptions.Center;
            tmp.sortingOrder = 20;
            return tmp;
        }

        /// <summary>
        /// Displays the diegetic title plaque / box and zooms the camera into the title viewpoint.
        /// </summary>
        public void ShowTitleDiorama(bool snapImmediately = false)
        {
            EnsureBuilt();
            _isTitleActive = true;
            if (dioramaRoot != null)
            {
                dioramaRoot.gameObject.SetActive(true);
            }
            if (interactionController != null)
            {
                interactionController.InteractionEnabled = false;
            }
            if (cameraSpring != null)
            {
                cameraSpring.SetTarget(titleCameraPosition, Quaternion.Euler(titleCameraEuler), Vector3.one, snapImmediately);
            }
            else if (mainCamera != null && snapImmediately)
            {
                mainCamera.transform.position = titleCameraPosition;
                mainCamera.transform.rotation = Quaternion.Euler(titleCameraEuler);
            }
        }

        /// <summary>
        /// Clicking Play Match smoothly transitions the camera (SecondOrderSpring) to the active 2.5D gameplay
        /// viewpoint and triggers the Mulligan sequence.
        /// </summary>
        public void PressPlayMatchButton(bool snapImmediately = false, bool triggerMulligan = true)
        {
            EnsureBuilt();
            if (playMatchButtonSpring != null)
            {
                playMatchButtonSpring.TriggerImpactSquashPunch();
            }
            if (audioAndProps != null)
            {
                audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.ButtonClick);
            }

            HideTitleDioramaToGameplay(snapImmediately, triggerMulligan);
            OnPlayMatchLaunched?.Invoke();
        }

        public void HideTitleDioramaToGameplay(bool snapImmediately = false, bool triggerMulligan = true)
        {
            EnsureBuilt();
            _isTitleActive = false;
            if (dioramaRoot != null)
            {
                dioramaRoot.gameObject.SetActive(false);
            }

            if (cameraSpring != null)
            {
                cameraSpring.SetTarget(gameplayCameraPosition, Quaternion.Euler(gameplayCameraEuler), Vector3.one, snapImmediately);
            }
            else if (mainCamera != null && snapImmediately)
            {
                mainCamera.transform.position = gameplayCameraPosition;
                mainCamera.transform.rotation = Quaternion.Euler(gameplayCameraEuler);
            }

            if (triggerMulligan && mulliganController != null)
            {
                mulliganController.BeginOpeningMulligan(snapImmediately);
            }
            else if (interactionController != null)
            {
                interactionController.InteractionEnabled = true;
            }
        }

        public bool ToggleAiSpeedButton()
        {
            EnsureBuilt();
            _fastAiSpeed = !_fastAiSpeed;
            if (aiSpeedButtonSpring != null)
            {
                aiSpeedButtonSpring.TriggerImpactSquashPunch();
            }
            if (audioAndProps != null)
            {
                audioAndProps.PlaySound(TabletopAudioAndProps.SoundEffectType.ButtonClick);
            }
            if (aiSpeedButtonLabel != null)
            {
                aiSpeedButtonLabel.text = _fastAiSpeed
                    ? "AI SPEED: FAST (1.8x)"
                    : "AI SPEED: STANDARD (1.0x)";
            }

            Heav3ns.UnityBridge.VisualEventQueue queue = FindAnyObjectByType<Heav3ns.UnityBridge.VisualEventQueue>();
            if (queue != null)
            {
                queue.PlaybackSpeedMultiplier = _fastAiSpeed ? 1.8f : 1.0f;
            }
            return _fastAiSpeed;
        }

        public void PressQuitButton()
        {
            EnsureBuilt();
            if (quitButtonSpring != null)
            {
                quitButtonSpring.TriggerImpactSquashPunch();
            }
            if (!Application.isEditor)
            {
                Application.Quit();
            }
        }

        private void Update()
        {
            if (!_isTitleActive)
            {
                return;
            }

            if (UnityEngine.Input.GetMouseButtonDown(0))
            {
                Camera cam = mainCamera != null ? mainCamera : Camera.main;
                if (cam != null)
                {
                    Ray ray = cam.ScreenPointToRay(UnityEngine.Input.mousePosition);
                    if (Physics.Raycast(ray, out RaycastHit hit, 100f))
                    {
                        TryClickTitleControl(hit.transform);
                    }
                }
            }
        }

        public bool TryClickTitleControl(Transform hitTransform)
        {
            EnsureBuilt();
            if (hitTransform == null || !_isTitleActive)
            {
                return false;
            }

            if (playMatchButtonTransform != null &&
                (hitTransform == playMatchButtonTransform || hitTransform.IsChildOf(playMatchButtonTransform)))
            {
                PressPlayMatchButton(false, true);
                return true;
            }

            if (aiSpeedButtonTransform != null &&
                (hitTransform == aiSpeedButtonTransform || hitTransform.IsChildOf(aiSpeedButtonTransform)))
            {
                ToggleAiSpeedButton();
                return true;
            }

            if (quitButtonTransform != null &&
                (hitTransform == quitButtonTransform || hitTransform.IsChildOf(quitButtonTransform)))
            {
                PressQuitButton();
                return true;
            }

            return false;
        }
    }
}

