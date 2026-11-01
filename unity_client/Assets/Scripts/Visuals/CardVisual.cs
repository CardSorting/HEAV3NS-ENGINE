using System;
using TMPro;
using UnityEngine;

namespace Heav3ns.Visuals
{
    /// <summary>
    /// Represents a physical 3D/2.5D card actor on the EnchantedTableStage.
    /// Backed by Assets/Prefabs/CardActor.prefab with front/back faces, ornate frame border,
    /// artwork slot, dynamic shadow, BoxCollider for raycast picking, SecondOrderSpring physics,
    /// custom shaders (Heav3ns/CardBorderGlow and Heav3ns/CardSurfaceFoil) driven by MaterialPropertyBlock,
    /// and world-space TextMeshPro labels for Cost, Attack/Power, Health/Defense, Title, and Rules Text.
    /// </summary>
    [DisallowMultipleComponent]
    [RequireComponent(typeof(BoxCollider))]
    [RequireComponent(typeof(SecondOrderSpring))]
    public class CardVisual : MonoBehaviour
    {
        public enum BorderHighlightState
        {
            None = 0,
            PlayableInHand = 1,
            ReadyInSequenceRow = 2,
            ValidTargetUnderArrow = 3
        }

        [Header("Card Identity & State")]
        [SerializeField] private int cardUid;
        [SerializeField] private string cardSpecId = string.Empty;
        [SerializeField] private int ownerSeat;
        [SerializeField] private string cardTitle = "Card";
        [SerializeField] private string cardType = "Character";
        [SerializeField] private int cost = 1;
        [SerializeField] private int attackPower = 1;
        [SerializeField] private int healthDefense = 1;
        [SerializeField] private string rulesText = string.Empty;
        [SerializeField] private bool isFaceDown;
        [SerializeField] private bool isPlayable = true;
        [SerializeField] private bool isTargetedHighlight;
        [SerializeField] private bool markedForMulligan;
        [SerializeField] private float dissolveAmount;
        [SerializeField] private string currentZone = "hand";

        [Header("3D Visual Sub-Objects")]
        [SerializeField] private Transform frontFaceRoot;
        [SerializeField] private Transform backFaceRoot;
        [SerializeField] private Renderer frameBorderRenderer;
        [SerializeField] private Renderer artworkSlotRenderer;
        [SerializeField] private Renderer backFaceRenderer;
        [SerializeField] private Transform shadowTransform;

        [Header("World-Space TextMeshPro Labels")]
        [SerializeField] private TMP_Text costLabel;
        [SerializeField] private TMP_Text attackPowerLabel;
        [SerializeField] private TMP_Text healthDefenseLabel;
        [SerializeField] private TMP_Text titleLabel;
        [SerializeField] private TMP_Text rulesTextLabel;
        [SerializeField] private TMP_Text mulliganBadgeLabel;

        private BoxCollider _boxCollider;
        private SecondOrderSpring _spring;
        private MaterialPropertyBlock _propBlock;
        private BorderHighlightState _activeHighlightState = BorderHighlightState.None;

        public int CardUid => cardUid;
        public string CardSpecId => cardSpecId;
        public int OwnerSeat => ownerSeat;
        public string CardTitle => cardTitle;
        public string CardType => cardType;
        public int Cost => cost;
        public int AttackPower => attackPower;
        public int HealthDefense => healthDefense;
        public string RulesText => rulesText;
        public bool IsFaceDown => isFaceDown;
        public bool IsPlayable => isPlayable;
        public bool IsTargetedHighlight => isTargetedHighlight;
        public bool MarkedForMulligan => markedForMulligan;
        public float DissolveAmount => dissolveAmount;
        public string CurrentZone => currentZone;
        public BorderHighlightState ActiveHighlightState => _activeHighlightState;

        public Renderer FrameBorderRenderer => frameBorderRenderer;
        public Renderer ArtworkSlotRenderer => artworkSlotRenderer;
        public Renderer BackFaceRenderer => backFaceRenderer;

        public BoxCollider Collider => _boxCollider != null ? _boxCollider : (_boxCollider = GetComponent<BoxCollider>());
        public SecondOrderSpring Spring => _spring != null ? _spring : (_spring = GetComponent<SecondOrderSpring>());

        public TMP_Text CostLabel => costLabel;
        public TMP_Text AttackPowerLabel => attackPowerLabel;
        public TMP_Text HealthDefenseLabel => healthDefenseLabel;
        public TMP_Text TitleLabel => titleLabel;
        public TMP_Text RulesTextLabel => rulesTextLabel;
        public TMP_Text MulliganBadgeLabel => mulliganBadgeLabel;

        private void Awake()
        {
            EnsureReferences();
            RefreshVisuals();
        }

        private void LateUpdate()
        {
            UpdateFoilTiltFromSpring();
        }

        public void EnsureReferences()
        {
            if (_boxCollider == null)
            {
                _boxCollider = GetComponent<BoxCollider>();
            }
            if (_spring == null)
            {
                _spring = GetComponent<SecondOrderSpring>();
            }
            if (shadowTransform == null)
            {
                Transform foundShadow = transform.Find("ShadowQuad");
                if (foundShadow != null)
                {
                    shadowTransform = foundShadow;
                }
            }
            if (_spring != null && shadowTransform != null)
            {
                _spring.ConfigureShadow(shadowTransform);
            }
            if (frontFaceRoot == null)
            {
                frontFaceRoot = transform.Find("FrontFace");
            }
            if (backFaceRoot == null)
            {
                backFaceRoot = transform.Find("BackFace");
            }
            if (frameBorderRenderer == null)
            {
                Transform frameTr = transform.Find("OrnateFrameBorder");
                if (frameTr != null) frameBorderRenderer = frameTr.GetComponent<Renderer>();
            }
            if (artworkSlotRenderer == null && frontFaceRoot != null)
            {
                Transform artTr = frontFaceRoot.Find("ArtworkSlot");
                if (artTr != null) artworkSlotRenderer = artTr.GetComponent<Renderer>();
            }
            if (backFaceRenderer == null && backFaceRoot != null)
            {
                backFaceRenderer = backFaceRoot.GetComponent<Renderer>();
            }

            if (frontFaceRoot != null)
            {
                if (costLabel == null) costLabel = FindTmpChild(frontFaceRoot, "CostLabel");
                if (attackPowerLabel == null) attackPowerLabel = FindTmpChild(frontFaceRoot, "AttackPowerLabel");
                if (healthDefenseLabel == null) healthDefenseLabel = FindTmpChild(frontFaceRoot, "HealthDefenseLabel");
                if (titleLabel == null) titleLabel = FindTmpChild(frontFaceRoot, "TitleLabel");
                if (rulesTextLabel == null) rulesTextLabel = FindTmpChild(frontFaceRoot, "RulesTextLabel");
                if (mulliganBadgeLabel == null) mulliganBadgeLabel = FindTmpChild(frontFaceRoot, "MulliganBadgeLabel");
            }

            EnsureCustomShadersAssigned();
        }

        private void EnsureCustomShadersAssigned()
        {
            if (frameBorderRenderer != null)
            {
                Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
                if (borderMat != null && frameBorderRenderer.sharedMaterial != borderMat)
                {
                    frameBorderRenderer.sharedMaterial = borderMat;
                }
            }
            if (artworkSlotRenderer != null)
            {
                Material foilMat = CardArtRegistry.GetSharedSurfaceFoilMaterial();
                if (foilMat != null && artworkSlotRenderer.sharedMaterial != foilMat)
                {
                    artworkSlotRenderer.sharedMaterial = foilMat;
                }
            }
            if (backFaceRenderer != null)
            {
                Material backMat = CardArtRegistry.GetSharedCardBackMaterial();
                if (backMat != null && backFaceRenderer.sharedMaterial != backMat)
                {
                    backFaceRenderer.sharedMaterial = backMat;
                }
            }
        }

        private static TMP_Text FindTmpChild(Transform root, string childName)
        {
            Transform child = root.Find(childName);
            return child != null ? child.GetComponent<TMP_Text>() : null;
        }

        public void BindWireReferences(
            Transform frontRoot,
            Transform backRoot,
            Renderer frameRenderer,
            Renderer artRenderer,
            Renderer backRend,
            Transform shadowTr,
            TMP_Text costTmp,
            TMP_Text attackTmp,
            TMP_Text healthTmp,
            TMP_Text titleTmp,
            TMP_Text rulesTmp,
            TMP_Text mulliganTmp = null)
        {
            frontFaceRoot = frontRoot;
            backFaceRoot = backRoot;
            frameBorderRenderer = frameRenderer;
            artworkSlotRenderer = artRenderer;
            backFaceRenderer = backRend;
            shadowTransform = shadowTr;
            costLabel = costTmp;
            attackPowerLabel = attackTmp;
            healthDefenseLabel = healthTmp;
            titleLabel = titleTmp;
            rulesTextLabel = rulesTmp;
            mulliganBadgeLabel = mulliganTmp;

            if (Spring != null && shadowTransform != null)
            {
                Spring.ConfigureShadow(shadowTransform);
            }
            EnsureCustomShadersAssigned();
        }

        /// <summary>
        /// Binds data from the Rust engine's serialized card state or VisualEventRecord.
        /// </summary>
        public void ConfigureCard(
            int uid,
            string specId,
            int owner,
            string title,
            string type,
            int cardCost,
            string text,
            int power,
            int defense,
            bool faceDown,
            string zone = "hand",
            bool playable = true)
        {
            EnsureReferences();
            cardUid = uid;
            cardSpecId = specId ?? string.Empty;
            ownerSeat = owner;
            cardTitle = string.IsNullOrWhiteSpace(title) ? (faceDown ? "Hidden Card" : "Wonderland Card") : title;
            cardType = string.IsNullOrWhiteSpace(type) ? "Character" : type;
            cost = Mathf.Max(0, cardCost);
            rulesText = text ?? string.Empty;
            attackPower = Mathf.Max(0, power);
            healthDefense = Mathf.Max(1, defense);
            isFaceDown = faceDown;
            currentZone = string.IsNullOrWhiteSpace(zone) ? "hand" : zone;
            isPlayable = playable;
            dissolveAmount = 0f;

            gameObject.name = faceDown
                ? $"CardActor_Seat{owner}_{zone}_Hidden_{(uid != 0 ? uid.ToString() : "X")}"
                : $"CardActor_{uid}_{cardSpecId}_{cardTitle.Replace(" ", "")}";

            RefreshVisuals();
        }

        public void SetFaceDown(bool faceDown)
        {
            isFaceDown = faceDown;
            RefreshVisuals();
        }

        public void SetZone(string zone)
        {
            currentZone = zone;
            RefreshVisuals();
        }

        public void SetPlayable(bool playable)
        {
            isPlayable = playable;
            RefreshVisuals();
        }

        public void SetTargetedHighlight(bool targeted)
        {
            if (isTargetedHighlight == targeted)
            {
                return;
            }
            isTargetedHighlight = targeted;
            RefreshVisuals();
        }

        public void SetMulliganMarked(bool marked)
        {
            markedForMulligan = marked;
            if (mulliganBadgeLabel == null && frontFaceRoot != null)
            {
                mulliganBadgeLabel = CreateWorldTmpLabel(
                    frontFaceRoot,
                    "MulliganBadgeLabel",
                    "REPLACE / REDRAW",
                    new Vector3(0f, 0.014f, 0.05f),
                    new Vector2(1.15f, 0.34f),
                    1.35f,
                    new Color(1.0f, 0.35f, 0.28f, 1f),
                    TextAlignmentOptions.Center);
            }
            if (mulliganBadgeLabel != null)
            {
                mulliganBadgeLabel.gameObject.SetActive(markedForMulligan);
            }
            RefreshVisuals();
        }

        /// <summary>
        /// Sets the noise-driven burn/dissolve threshold (_DissolveAmount 0.0 -> 1.0) on the card's
        /// artwork foil shader, frame border shader, and back face via MaterialPropertyBlock.
        /// </summary>
        public void SetDissolveAmount(float amount)
        {
            dissolveAmount = Mathf.Clamp01(amount);
            EnsureReferences();
            if (_propBlock == null)
            {
                _propBlock = new MaterialPropertyBlock();
            }

            if (frameBorderRenderer != null)
            {
                frameBorderRenderer.GetPropertyBlock(_propBlock);
                _propBlock.SetFloat("_DissolveAmount", dissolveAmount);
                frameBorderRenderer.SetPropertyBlock(_propBlock);
            }

            if (artworkSlotRenderer != null)
            {
                artworkSlotRenderer.GetPropertyBlock(_propBlock);
                _propBlock.SetFloat("_DissolveAmount", dissolveAmount);
                artworkSlotRenderer.SetPropertyBlock(_propBlock);
            }

            if (backFaceRenderer != null)
            {
                backFaceRenderer.GetPropertyBlock(_propBlock);
                _propBlock.SetFloat("_DissolveAmount", dissolveAmount);
                backFaceRenderer.SetPropertyBlock(_propBlock);
            }

            bool showLabels = dissolveAmount < 0.72f && !isFaceDown;
            if (frontFaceRoot != null)
            {
                frontFaceRoot.gameObject.SetActive(!isFaceDown);
            }
            if (costLabel != null) costLabel.enabled = showLabels;
            if (titleLabel != null) titleLabel.enabled = showLabels;
            if (rulesTextLabel != null) rulesTextLabel.enabled = showLabels;
            if (attackPowerLabel != null) attackPowerLabel.enabled = showLabels;
            if (healthDefenseLabel != null) healthDefenseLabel.enabled = showLabels;
        }

        public void UpdateFoilTiltFromSpring()
        {
            if (artworkSlotRenderer == null || isFaceDown || _spring == null)
            {
                return;
            }
            if (_propBlock == null)
            {
                _propBlock = new MaterialPropertyBlock();
            }
            artworkSlotRenderer.GetPropertyBlock(_propBlock);
            _propBlock.SetVector("_TiltAngles", new Vector4(_spring.TiltPitchDegrees, _spring.TiltRollDegrees, 0f, 0f));
            artworkSlotRenderer.SetPropertyBlock(_propBlock);
        }

        public void PulseStatBadges()
        {
            if (Spring != null)
            {
                Spring.TriggerImpactSquashPunch();
            }
        }

        public void RefreshVisuals()
        {
            EnsureReferences();

            if (frontFaceRoot != null)
            {
                frontFaceRoot.gameObject.SetActive(!isFaceDown);
            }
            if (backFaceRoot != null)
            {
                backFaceRoot.gameObject.SetActive(true);
            }
            if (mulliganBadgeLabel != null)
            {
                mulliganBadgeLabel.gameObject.SetActive(markedForMulligan && !isFaceDown);
            }

            if (costLabel != null)
            {
                costLabel.text = isFaceDown ? "?" : cost.ToString();
            }
            if (attackPowerLabel != null)
            {
                attackPowerLabel.text = isFaceDown ? "?" : attackPower.ToString();
            }
            if (healthDefenseLabel != null)
            {
                healthDefenseLabel.text = isFaceDown ? "?" : healthDefense.ToString();
            }
            if (titleLabel != null)
            {
                titleLabel.text = isFaceDown ? "Wonderland" : cardTitle;
            }
            if (rulesTextLabel != null)
            {
                rulesTextLabel.text = isFaceDown ? string.Empty : rulesText;
            }

            if (_propBlock == null)
            {
                _propBlock = new MaterialPropertyBlock();
            }

            // Compute BorderHighlightState for CardBorderGlow.shader
            if (isTargetedHighlight || markedForMulligan)
            {
                _activeHighlightState = BorderHighlightState.ValidTargetUnderArrow;
            }
            else if (!isFaceDown && ownerSeat == 0 && currentZone == "hand" && isPlayable)
            {
                _activeHighlightState = BorderHighlightState.PlayableInHand;
            }
            else if (!isFaceDown && ownerSeat == 0 && currentZone == "field")
            {
                _activeHighlightState = BorderHighlightState.ReadyInSequenceRow;
            }
            else
            {
                _activeHighlightState = BorderHighlightState.None;
            }

            if (frameBorderRenderer != null)
            {
                Color frameColor = isFaceDown
                    ? new Color(0.26f, 0.20f, 0.36f, 1f)
                    : (isPlayable && ownerSeat == 0 && currentZone == "hand"
                        ? new Color(0.88f, 0.74f, 0.32f, 1f)
                        : (ownerSeat == 0
                            ? new Color(0.28f, 0.52f, 0.78f, 1f)
                            : new Color(0.75f, 0.25f, 0.32f, 1f)));

                Color glowPrimary = Color.black;
                Color glowSecondary = Color.black;
                float glowIntensity = 0f;
                float pulseSpeed = 3.2f;

                switch (_activeHighlightState)
                {
                    case BorderHighlightState.PlayableInHand:
                        // Subtle pulsing emerald (#34D399) / gold (#FBBF24) emission
                        glowPrimary = new Color(0.204f, 0.827f, 0.600f, 1f);
                        glowSecondary = new Color(0.984f, 0.749f, 0.141f, 1f);
                        glowIntensity = 0.85f;
                        pulseSpeed = 3.2f;
                        break;
                    case BorderHighlightState.ReadyInSequenceRow:
                        // Crisp cyan / gold aura
                        glowPrimary = new Color(0.18f, 0.88f, 0.98f, 1f);
                        glowSecondary = new Color(0.98f, 0.82f, 0.24f, 1f);
                        glowIntensity = 0.92f;
                        pulseSpeed = 4.0f;
                        break;
                    case BorderHighlightState.ValidTargetUnderArrow:
                        // High-contrast crimson / amber pulse
                        glowPrimary = new Color(0.96f, 0.18f, 0.24f, 1f);
                        glowSecondary = new Color(1.0f, 0.68f, 0.12f, 1f);
                        glowIntensity = 1.35f;
                        pulseSpeed = 6.5f;
                        break;
                }

                frameBorderRenderer.GetPropertyBlock(_propBlock);
                _propBlock.SetColor("_BaseColor", frameColor);
                _propBlock.SetColor("_Color", frameColor);
                _propBlock.SetColor("_GlowColor", glowPrimary);
                _propBlock.SetColor("_GlowSecondaryColor", glowSecondary);
                _propBlock.SetFloat("_GlowIntensity", glowIntensity);
                _propBlock.SetFloat("_PulseSpeed", pulseSpeed);
                _propBlock.SetFloat("_HighlightState", (float)_activeHighlightState);
                _propBlock.SetFloat("_DissolveAmount", dissolveAmount);
                frameBorderRenderer.SetPropertyBlock(_propBlock);
            }

            if (artworkSlotRenderer != null && !isFaceDown)
            {
                Texture2D artTexture = CardArtRegistry.GetCardArtTexture(cardSpecId, cardTitle, cardType, ownerSeat);
                artworkSlotRenderer.GetPropertyBlock(_propBlock);
                if (artTexture != null)
                {
                    _propBlock.SetTexture("_MainTex", artTexture);
                }
                _propBlock.SetColor("_BaseColor", Color.white);
                _propBlock.SetColor("_Color", Color.white);
                _propBlock.SetFloat("_FoilIntensity", 0.52f);
                _propBlock.SetFloat("_DissolveAmount", dissolveAmount);
                if (_spring != null)
                {
                    _propBlock.SetVector("_TiltAngles", new Vector4(_spring.TiltPitchDegrees, _spring.TiltRollDegrees, 0f, 0f));
                }
                artworkSlotRenderer.SetPropertyBlock(_propBlock);
            }

            if (backFaceRenderer != null)
            {
                Texture2D backTex = CardArtRegistry.GetOrCreateCardBackTexture();
                backFaceRenderer.GetPropertyBlock(_propBlock);
                if (backTex != null)
                {
                    _propBlock.SetTexture("_MainTex", backTex);
                }
                _propBlock.SetColor("_BaseColor", Color.white);
                _propBlock.SetColor("_Color", Color.white);
                _propBlock.SetFloat("_DissolveAmount", dissolveAmount);
                backFaceRenderer.SetPropertyBlock(_propBlock);
            }
        }

        /// <summary>
        /// Constructs a procedural 3D/2.5D CardActor GameObject hierarchy with front/back faces,
        /// ornate frame border, artwork slot, dynamic shadow, BoxCollider, SecondOrderSpring,
        /// custom shaders (CardBorderGlow & CardSurfaceFoil), and world-space TextMeshPro labels.
        /// </summary>
        public static CardVisual ConstructProceduralCardObject(string objectName = "CardActor")
        {
            GameObject root = new GameObject(objectName);

            // Standard tabletop card dimensions: 1.35 width x 0.04 thickness x 1.90 height (laid flat on X-Z plane)
            BoxCollider collider = root.AddComponent<BoxCollider>();
            collider.center = Vector3.zero;
            collider.size = new Vector3(1.36f, 0.08f, 1.92f);

            SecondOrderSpring spring = root.AddComponent<SecondOrderSpring>();
            CardVisual cardVisual = root.AddComponent<CardVisual>();

            // 1. Dynamic Drop Shadow Quad
            GameObject shadowGo = GameObject.CreatePrimitive(PrimitiveType.Quad);
            shadowGo.name = "ShadowQuad";
            DestroyColliderImmediate(shadowGo);
            shadowGo.transform.SetParent(root.transform, false);
            shadowGo.transform.localPosition = new Vector3(0.06f, -0.03f, -0.06f);
            shadowGo.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            shadowGo.transform.localScale = new Vector3(1.42f, 1.98f, 1f);
            Renderer shadowRenderer = shadowGo.GetComponent<Renderer>();
            ApplyColorToRenderer(shadowRenderer, new Color(0.02f, 0.02f, 0.04f, 0.65f));

            // 2. Ornate Frame Border (3D weighted cardboard box with CardBorderGlow.shader)
            GameObject frameGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
            frameGo.name = "OrnateFrameBorder";
            DestroyColliderImmediate(frameGo);
            frameGo.transform.SetParent(root.transform, false);
            frameGo.transform.localPosition = Vector3.zero;
            frameGo.transform.localScale = new Vector3(1.35f, 0.04f, 1.90f);
            Renderer frameRenderer = frameGo.GetComponent<Renderer>();
            Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
            if (borderMat != null) frameRenderer.sharedMaterial = borderMat;
            ApplyColorToRenderer(frameRenderer, new Color(0.82f, 0.68f, 0.30f, 1f));

            // 3. Front Face Root
            GameObject frontRoot = new GameObject("FrontFace");
            frontRoot.transform.SetParent(root.transform, false);
            frontRoot.transform.localPosition = new Vector3(0f, 0.022f, 0f);

            // Front parchment face plate
            GameObject frontPlate = GameObject.CreatePrimitive(PrimitiveType.Quad);
            frontPlate.name = "FrontParchmentPlate";
            DestroyColliderImmediate(frontPlate);
            frontPlate.transform.SetParent(frontRoot.transform, false);
            frontPlate.transform.localPosition = Vector3.zero;
            frontPlate.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            frontPlate.transform.localScale = new Vector3(1.24f, 1.78f, 1f);
            ApplyColorToRenderer(frontPlate.GetComponent<Renderer>(), new Color(0.14f, 0.13f, 0.19f, 1f));

            // Artwork Slot (with CardSurfaceFoil.shader)
            GameObject artSlot = GameObject.CreatePrimitive(PrimitiveType.Quad);
            artSlot.name = "ArtworkSlot";
            DestroyColliderImmediate(artSlot);
            artSlot.transform.SetParent(frontRoot.transform, false);
            artSlot.transform.localPosition = new Vector3(0f, 0.002f, 0.24f);
            artSlot.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            artSlot.transform.localScale = new Vector3(1.08f, 0.78f, 1f);
            Renderer artRenderer = artSlot.GetComponent<Renderer>();
            Material foilMat = CardArtRegistry.GetSharedSurfaceFoilMaterial();
            if (foilMat != null) artRenderer.sharedMaterial = foilMat;
            ApplyColorToRenderer(artRenderer, Color.white);

            // 4. Back Face Root (visible when rotated 180 deg around Z/X or when face-down)
            GameObject backRoot = GameObject.CreatePrimitive(PrimitiveType.Quad);
            backRoot.name = "BackFace";
            DestroyColliderImmediate(backRoot);
            backRoot.transform.SetParent(root.transform, false);
            backRoot.transform.localPosition = new Vector3(0f, -0.022f, 0f);
            backRoot.transform.localRotation = Quaternion.Euler(-90f, 0f, 0f);
            backRoot.transform.localScale = new Vector3(1.26f, 1.80f, 1f);
            Renderer backRenderer = backRoot.GetComponent<Renderer>();
            Material backMat = CardArtRegistry.GetSharedCardBackMaterial();
            if (backMat != null) backRenderer.sharedMaterial = backMat;
            ApplyColorToRenderer(backRenderer, new Color(0.28f, 0.16f, 0.38f, 1f));

            // 5. World-Space TextMeshPro Labels: Cost, Attack/Power, Health/Defense, Title, Rules Text, Mulligan Badge
            TextMeshPro costTmp = CreateWorldTmpLabel(
                frontRoot.transform,
                "CostLabel",
                "1",
                new Vector3(-0.48f, 0.006f, 0.74f),
                new Vector2(0.32f, 0.26f),
                2.2f,
                new Color(0.45f, 0.88f, 1.0f, 1f),
                TextAlignmentOptions.Center);

            TextMeshPro titleTmp = CreateWorldTmpLabel(
                frontRoot.transform,
                "TitleLabel",
                "Card Title",
                new Vector3(0.06f, 0.006f, 0.74f),
                new Vector2(0.84f, 0.26f),
                1.35f,
                new Color(0.98f, 0.95f, 0.86f, 1f),
                TextAlignmentOptions.Left);

            TextMeshPro rulesTmp = CreateWorldTmpLabel(
                frontRoot.transform,
                "RulesTextLabel",
                "Card rules text.",
                new Vector3(0f, 0.006f, -0.36f),
                new Vector2(1.08f, 0.58f),
                1.05f,
                new Color(0.90f, 0.88f, 0.84f, 1f),
                TextAlignmentOptions.TopLeft);

            TextMeshPro attackTmp = CreateWorldTmpLabel(
                frontRoot.transform,
                "AttackPowerLabel",
                "2",
                new Vector3(-0.46f, 0.006f, -0.75f),
                new Vector2(0.34f, 0.24f),
                1.85f,
                new Color(1.0f, 0.62f, 0.32f, 1f),
                TextAlignmentOptions.Center);

            TextMeshPro healthTmp = CreateWorldTmpLabel(
                frontRoot.transform,
                "HealthDefenseLabel",
                "3",
                new Vector3(0.46f, 0.006f, -0.75f),
                new Vector2(0.34f, 0.24f),
                1.85f,
                new Color(0.48f, 0.95f, 0.56f, 1f),
                TextAlignmentOptions.Center);

            TextMeshPro mulliganTmp = CreateWorldTmpLabel(
                frontRoot.transform,
                "MulliganBadgeLabel",
                "REPLACE / REDRAW",
                new Vector3(0f, 0.014f, 0.05f),
                new Vector2(1.15f, 0.34f),
                1.35f,
                new Color(1.0f, 0.35f, 0.28f, 1f),
                TextAlignmentOptions.Center);
            mulliganTmp.gameObject.SetActive(false);

            spring.ConfigureShadow(shadowGo.transform);
            cardVisual.BindWireReferences(
                frontRoot.transform,
                backRoot.transform,
                frameRenderer,
                artRenderer,
                backRenderer,
                shadowGo.transform,
                costTmp,
                attackTmp,
                healthTmp,
                titleTmp,
                rulesTmp,
                mulliganTmp);
            cardVisual.RefreshVisuals();

            return cardVisual;
        }

        private static TextMeshPro CreateWorldTmpLabel(
            Transform parent,
            string name,
            string defaultText,
            Vector3 localPos,
            Vector2 rectSize,
            float fontSize,
            Color textColor,
            TextAlignmentOptions alignment)
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
            tmp.text = defaultText;
            tmp.fontSize = fontSize;
            tmp.color = textColor;
            tmp.alignment = alignment;
            tmp.textWrappingMode = TextWrappingModes.Normal;
            tmp.overflowMode = TextOverflowModes.Ellipsis;
            tmp.sortingOrder = 10;
            return tmp;
        }

        private static void DestroyColliderImmediate(GameObject go)
        {
            Collider col = go.GetComponent<Collider>();
            if (col != null)
            {
                if (Application.isPlaying)
                {
                    Destroy(col);
                }
                else
                {
                    DestroyImmediate(col);
                }
            }
        }

        private static void ApplyColorToRenderer(Renderer renderer, Color color)
        {
            if (renderer == null)
            {
                return;
            }
            MaterialPropertyBlock block = new MaterialPropertyBlock();
            renderer.GetPropertyBlock(block);
            block.SetColor("_BaseColor", color);
            block.SetColor("_Color", color);
            renderer.SetPropertyBlock(block);
        }
    }
}
