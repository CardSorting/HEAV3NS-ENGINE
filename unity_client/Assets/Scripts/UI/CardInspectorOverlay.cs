using System;
using System.Collections.Generic;
using Heav3ns.Visuals;
using TMPro;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>
    /// Phase 3 Big-Card Inspector (CardInspectorOverlay.cs).
    /// Right-clicking (or long-hovering for 0.6s) any revealed CardVisual on the board or in hand
    /// spawns a crisp, un-tilted 2.2x inspection preview on the left side of the screen alongside
    /// floating keyword tooltip callouts (explaining Sequence rules, Shield triggers, or keywords).
    /// </summary>
    [DisallowMultipleComponent]
    public class CardInspectorOverlay : MonoBehaviour
    {
        [Header("Inspector Configuration")]
        [SerializeField] private float longHoverThresholdSeconds = 0.6f;
        [SerializeField] private float inspectionScaleMultiplier = 2.2f;
        [SerializeField] private Vector3 inspectorWorldPosition = new Vector3(-5.85f, 3.85f, -2.45f);
        [SerializeField] private Vector3 inspectorEulerAngles = new Vector3(-34f, 0f, 0f);

        [Header("Inspector Hierarchy")]
        [SerializeField] private Transform inspectorRoot;
        [SerializeField] private CardVisual previewCardActor;
        [SerializeField] private Transform keywordTooltipsContainer;
        [SerializeField] private TMP_Text keywordCalloutLabel;

        private CardVisual _inspectedSourceCard;
        private CardVisual _currentHoverCandidate;
        private float _hoverElapsedSeconds;
        private bool _isShowing;

        public bool IsShowing => _isShowing && inspectorRoot != null && inspectorRoot.gameObject.activeSelf;
        public CardVisual InspectedSourceCard => _inspectedSourceCard;
        public CardVisual PreviewCardActor => previewCardActor;
        public TMP_Text KeywordCalloutLabel => keywordCalloutLabel;
        public float InspectionScaleMultiplier => inspectionScaleMultiplier;
        public float LongHoverThresholdSeconds => longHoverThresholdSeconds;

        private void Awake()
        {
            EnsureBuilt();
        }

        public void EnsureBuilt()
        {
            GameObject stage = GameObject.Find("EnchantedTableStage");
            Transform parent = stage != null ? stage.transform : transform;

            if (inspectorRoot == null)
            {
                Transform existing = parent.Find("CardInspectorOverlayRoot");
                if (existing != null)
                {
                    inspectorRoot = existing;
                    previewCardActor = existing.GetComponentInChildren<CardVisual>(true);
                    keywordTooltipsContainer = existing.Find("KeywordTooltipCalloutBox");
                    if (keywordTooltipsContainer != null)
                    {
                        keywordCalloutLabel = keywordTooltipsContainer.GetComponentInChildren<TMP_Text>(true);
                    }
                }
                else
                {
                    GameObject rootGo = new GameObject("CardInspectorOverlayRoot");
                    rootGo.transform.SetParent(parent, false);
                    rootGo.transform.localPosition = inspectorWorldPosition;
                    rootGo.transform.localRotation = Quaternion.Euler(inspectorEulerAngles);
                    inspectorRoot = rootGo.transform;

                    previewCardActor = CardVisual.ConstructProceduralCardObject("InspectorPreviewCardActor");
                    Collider col = previewCardActor.GetComponent<Collider>();
                    if (col != null) col.enabled = false;
                    previewCardActor.transform.SetParent(inspectorRoot, false);
                    previewCardActor.transform.localPosition = Vector3.zero;
                    previewCardActor.transform.localRotation = Quaternion.identity;
                    previewCardActor.transform.localScale = Vector3.one * inspectionScaleMultiplier;
                    previewCardActor.Spring.SetTarget(Vector3.zero, Quaternion.identity, Vector3.one * inspectionScaleMultiplier, true);

                    // Floating Keyword Tooltip Callout Panel next to the 2.2x preview card
                    GameObject tooltipBox = GameObject.CreatePrimitive(PrimitiveType.Cube);
                    tooltipBox.name = "KeywordTooltipCalloutBox";
                    Collider boxCol = tooltipBox.GetComponent<Collider>();
                    if (boxCol != null)
                    {
                        if (Application.isPlaying) Destroy(boxCol);
                        else DestroyImmediate(boxCol);
                    }
                    tooltipBox.transform.SetParent(inspectorRoot, false);
                    tooltipBox.transform.localPosition = new Vector3(2.95f, 0.02f, 0.15f);
                    tooltipBox.transform.localRotation = Quaternion.identity;
                    tooltipBox.transform.localScale = new Vector3(2.65f, 0.04f, 2.85f);
                    keywordTooltipsContainer = tooltipBox.transform;

                    Renderer boxRend = tooltipBox.GetComponent<Renderer>();
                    if (boxRend != null)
                    {
                        Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
                        if (borderMat != null) boxRend.sharedMaterial = borderMat;
                        MaterialPropertyBlock block = new MaterialPropertyBlock();
                        boxRend.GetPropertyBlock(block);
                        Color panelCol = new Color(0.12f, 0.11f, 0.18f, 0.96f);
                        block.SetColor("_BaseColor", panelCol);
                        block.SetColor("_Color", panelCol);
                        boxRend.SetPropertyBlock(block);
                    }

                    GameObject labelGo = new GameObject("KeywordCalloutText");
                    labelGo.transform.SetParent( inspectorRoot, false );
                    labelGo.transform.localPosition = new Vector3(2.95f, 0.06f, 0.15f);
                    labelGo.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
                    TextMeshPro tmp = labelGo.AddComponent<TextMeshPro>();
                    RectTransform rt = labelGo.GetComponent<RectTransform>();
                    if (rt != null) rt.sizeDelta = new Vector2(2.45f, 2.55f);
                    tmp.fontSize = 1.25f;
                    tmp.color = new Color(0.96f, 0.92f, 0.78f, 1f);
                    tmp.alignment = TextAlignmentOptions.TopLeft;
                    tmp.textWrappingMode = TextWrappingModes.Normal;
                    tmp.sortingOrder = 30;
                    keywordCalloutLabel = tmp;

                    inspectorRoot.gameObject.SetActive(false);
                }
            }
        }

        /// <summary>
        /// Shows the crisp, un-tilted 2.2x inspection preview for any revealed CardVisual
        /// and populates floating keyword tooltip callouts.
        /// Returns true if the card is revealed and the overlay opened.
        /// </summary>
        public bool InspectCard(CardVisual targetCard)
        {
            EnsureBuilt();
            if (targetCard == null || targetCard.IsFaceDown)
            {
                HideInspection();
                return false;
            }

            _inspectedSourceCard = targetCard;
            _isShowing = true;

            if (inspectorRoot != null)
            {
                inspectorRoot.gameObject.SetActive(true);
            }

            if (previewCardActor != null)
            {
                previewCardActor.ConfigureCard(
                    targetCard.CardUid,
                    targetCard.CardSpecId,
                    targetCard.OwnerSeat,
                    targetCard.CardTitle,
                    targetCard.CardType,
                    targetCard.Cost,
                    targetCard.RulesText,
                    targetCard.AttackPower,
                    targetCard.HealthDefense,
                    false,
                    "inspector",
                    false);
                previewCardActor.transform.localPosition = Vector3.zero;
                previewCardActor.transform.localRotation = Quaternion.identity;
                previewCardActor.transform.localScale = Vector3.one * inspectionScaleMultiplier;
                previewCardActor.Spring.SetTarget(Vector3.zero, Quaternion.identity, Vector3.one * inspectionScaleMultiplier, true);
            }

            if (keywordCalloutLabel != null)
            {
                keywordCalloutLabel.text = BuildKeywordCalloutText(targetCard);
            }

            return true;
        }

        public void HideInspection()
        {
            _isShowing = false;
            _inspectedSourceCard = null;
            _hoverElapsedSeconds = 0f;
            if (inspectorRoot != null)
            {
                inspectorRoot.gameObject.SetActive(false);
            }
        }

        /// <summary>
        /// Tracks long-hover duration (0.6s threshold) on a revealed card to automatically open the inspector.
        /// </summary>
        public void TickHoverCandidate(CardVisual hoveredCard, float deltaTime, bool rightClickPressed = false)
        {
            EnsureBuilt();

            if (rightClickPressed)
            {
                if (hoveredCard != null && !hoveredCard.IsFaceDown)
                {
                    if (_isShowing && _inspectedSourceCard == hoveredCard)
                    {
                        HideInspection();
                    }
                    else
                    {
                        InspectCard(hoveredCard);
                    }
                }
                else
                {
                    HideInspection();
                }
                return;
            }

            if (hoveredCard == null || hoveredCard.IsFaceDown)
            {
                _currentHoverCandidate = null;
                _hoverElapsedSeconds = 0f;
                return;
            }

            if (_currentHoverCandidate != hoveredCard)
            {
                _currentHoverCandidate = hoveredCard;
                _hoverElapsedSeconds = Mathf.Max(0f, deltaTime);
            }
            else
            {
                _hoverElapsedSeconds += Mathf.Max(0f, deltaTime);
            }

            if (_hoverElapsedSeconds >= longHoverThresholdSeconds && (!_isShowing || _inspectedSourceCard != hoveredCard))
            {
                InspectCard(hoveredCard);
            }
        }

        public static string BuildKeywordCalloutText(CardVisual card)
        {
            if (card == null)
            {
                return string.Empty;
            }

            List<string> sections = new List<string>();
            sections.Add($"<b>{card.CardTitle.ToUpperInvariant()}</b>\n<color=#88CCFF>{card.CardType} • Cost {card.Cost}</color>");

            string rules = card.RulesText ?? string.Empty;
            string lower = rules.ToLowerInvariant();

            // Always explain Sequence placement rules for context
            sections.Add("<b>SEQUENCE RULE:</b> Cards placed into the Sequence Row resolve in order when closed.");

            if (lower.Contains("shield trigger"))
            {
                sections.Add("<b>SHIELD TRIGGER:</b> When this card is revealed from your Shield Row by an attack, its Shield Trigger resolves for free.");
            }
            if (lower.Contains("counter") || string.Equals(card.CardType, "Counter", StringComparison.OrdinalIgnoreCase))
            {
                sections.Add("<b>COUNTER:</b> Played during an opponent's response window to cancel an Action or Combo.");
            }
            if (lower.Contains("response"))
            {
                sections.Add("<b>RESPONSE:</b> Can be played during response windows or on your own turn.");
            }
            if (lower.Contains("combo"))
            {
                sections.Add("<b>COMBO:</b> Gains bonus effects when sufficient supporting cards are present in the active Sequence.");
            }
            if (lower.Contains("return") || lower.Contains("bounce"))
            {
                sections.Add("<b>RETURN TO HAND:</b> Moves a stationed Character from the Sequence Row back to its owner's hand.");
            }

            return string.Join("\n\n", sections);
        }
    }
}
