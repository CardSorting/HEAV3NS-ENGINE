using System;
using System.Collections.Generic;
using Heav3ns.UnityBridge;
using Heav3ns.Visuals;
using TMPro;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>
    /// Phase 3 Diegetic Turn History Ribbon (ActionHistoryLog.cs).
    /// Mounts a vertical parchment/brass history bar along the left rim of EnchantedTableStage
    /// displaying the last 6 resolved VisualEvent entries (Card Played, Attack, Shield Broken)
    /// as compact hoverable tiles so the player can review what White Rabbit did during the AI turn.
    /// </summary>
    [DisallowMultipleComponent]
    public class ActionHistoryLog : MonoBehaviour
    {
        [Serializable]
        public class HistoryTileEntry
        {
            public VisualEventRecord eventRecord;
            public string summaryTitle;
            public string summaryDetail;
            public CardVisual snapshotCardVisual;
        }

        public const int MaxHistoryEntries = 6;

        [Header("Ribbon Layout")]
        [SerializeField] private Vector3 ribbonBasePosition = new Vector3(-7.75f, 0.24f, 0f);
        [SerializeField] private float tileStepZ = -0.82f;
        [SerializeField] private float firstTileZ = 2.05f;

        [Header("References")]
        [SerializeField] private Transform ribbonRoot;
        [SerializeField] private CardInspectorOverlay inspectorOverlay;

        private readonly List<HistoryTileEntry> _entries = new List<HistoryTileEntry>();
        private readonly List<Transform> _tileTransforms = new List<Transform>();
        private readonly List<Renderer> _tileRenderers = new List<Renderer>();
        private readonly List<TMP_Text> _tileLabels = new List<TMP_Text>();
        private MaterialPropertyBlock _propBlock;

        public IReadOnlyList<HistoryTileEntry> Entries => _entries;
        public IReadOnlyList<Transform> TileTransforms => _tileTransforms;
        public Transform RibbonRoot => ribbonRoot;

        private void Awake()
        {
            EnsureBuilt();
        }

        public void EnsureBuilt()
        {
            if (inspectorOverlay == null)
            {
                inspectorOverlay = FindAnyObjectByType<CardInspectorOverlay>();
            }

            GameObject stage = GameObject.Find("EnchantedTableStage");
            Transform parent = stage != null ? stage.transform : transform;

            if (ribbonRoot == null)
            {
                Transform existing = parent.Find("ActionHistoryRibbon");
                if (existing != null)
                {
                    ribbonRoot = existing;
                    RebuildTileReferencesFromExisting();
                }
                else
                {
                    GameObject rootGo = new GameObject("ActionHistoryRibbon");
                    rootGo.transform.SetParent(parent, false);
                    rootGo.transform.localPosition = ribbonBasePosition;
                    ribbonRoot = rootGo.transform;

                    // Vertical Parchment & Brass Backing Bar along the left rim of EnchantedTableStage
                    GameObject barGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
                    barGo.name = "BrassParchmentHistoryBar";
                    Collider barCol = barGo.GetComponent<Collider>();
                    if (barCol != null)
                    {
                        if (Application.isPlaying) Destroy(barCol);
                        else DestroyImmediate(barCol);
                    }
                    barGo.transform.SetParent(ribbonRoot, false);
                    barGo.transform.localPosition = new Vector3(0f, -0.04f, 0f);
                    barGo.transform.localScale = new Vector3(1.52f, 0.08f, 5.45f);
                    Renderer barRend = barGo.GetComponent<Renderer>();
                    if (barRend != null)
                    {
                        Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
                        if (borderMat != null) barRend.sharedMaterial = borderMat;
                        MaterialPropertyBlock block = new MaterialPropertyBlock();
                        barRend.GetPropertyBlock(block);
                        Color brassCol = new Color(0.38f, 0.30f, 0.18f, 1f);
                        block.SetColor("_BaseColor", brassCol);
                        block.SetColor("_Color", brassCol);
                        barRend.SetPropertyBlock(block);
                    }

                    // Create 6 compact hoverable tile slots
                    _tileTransforms.Clear();
                    _tileRenderers.Clear();
                    _tileLabels.Clear();

                    for (int i = 0; i < MaxHistoryEntries; i++)
                    {
                        GameObject tileGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
                        tileGo.name = $"HistoryTile_{i}";
                        tileGo.transform.SetParent(ribbonRoot, false);
                        tileGo.transform.localPosition = new Vector3(0f, 0.05f, firstTileZ + i * tileStepZ);
                        tileGo.transform.localScale = new Vector3(1.36f, 0.08f, 0.68f);

                        Renderer rend = tileGo.GetComponent<Renderer>();
                        if (rend != null)
                        {
                            Material borderMat = CardArtRegistry.GetSharedBorderGlowMaterial();
                            if (borderMat != null) rend.sharedMaterial = borderMat;
                        }

                        GameObject labelGo = new GameObject("TileLabel");
                        labelGo.transform.SetParent(tileGo.transform, false);
                        labelGo.transform.localPosition = new Vector3(0f, 0.58f, 0f);
                        labelGo.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
                        TextMeshPro tmp = labelGo.AddComponent<TextMeshPro>();
                        RectTransform rt = labelGo.GetComponent<RectTransform>();
                        if (rt != null) rt.sizeDelta = new Vector2(1.26f, 0.58f);
                        tmp.fontSize = 1.05f;
                        tmp.color = Color.white;
                        tmp.alignment = TextAlignmentOptions.Center;
                        tmp.textWrappingMode = TextWrappingModes.Normal;
                        tmp.overflowMode = TextOverflowModes.Ellipsis;
                        tmp.sortingOrder = 15;

                        _tileTransforms.Add(tileGo.transform);
                        _tileRenderers.Add(rend);
                        _tileLabels.Add(tmp);
                        tileGo.SetActive(false);
                    }
                }
            }
        }

        private void RebuildTileReferencesFromExisting()
        {
            _tileTransforms.Clear();
            _tileRenderers.Clear();
            _tileLabels.Clear();
            if (ribbonRoot == null) return;

            for (int i = 0; i < MaxHistoryEntries; i++)
            {
                Transform t = ribbonRoot.Find($"HistoryTile_{i}");
                if (t != null)
                {
                    _tileTransforms.Add(t);
                    _tileRenderers.Add(t.GetComponent<Renderer>());
                    _tileLabels.Add(t.GetComponentInChildren<TMP_Text>(true));
                }
            }
        }

        /// <summary>
        /// Evaluates a VisualEventRecord and records it if it represents a significant resolved action
        /// (Card Played / Unit Summoned / Spell Cast, Attack / Damage Dealt / Shield Revealed, Shield Broken / Unit Destroyed).
        /// Keeps the last 6 entries and updates the 3D hoverable tiles.
        /// </summary>
        public bool RecordVisualEvent(VisualEventRecord evt)
        {
            EnsureBuilt();
            if (!IsLoggableEvent(evt.eventType))
            {
                return false;
            }

            string actor = evt.seat == 0 ? "ALICE" : "RABBIT";
            string cardName = !string.IsNullOrWhiteSpace(evt.title) ? evt.title : evt.eventType;
            string badge;

            switch (evt.eventType)
            {
                case "CardPlayed":
                case "UnitPlaced":
                case "UnitSummoned":
                case "SpellCast":
                case "ShieldPlaced":
                    badge = $"[{actor}] PLAYED\n{cardName}";
                    break;
                case "AttackDeclared":
                case "CombatResolved":
                case "DamageDealt":
                    badge = evt.isShieldBreak
                        ? $"[{actor}] SHIELD HIT\n{cardName}"
                        : $"[{actor}] ATTACK\n{cardName}";
                    break;
                case "ShieldRevealed":
                case "ShieldBroken":
                    badge = $"[{actor}] SHIELD BROKEN\n{cardName}";
                    break;
                case "CardDestroyed":
                case "UnitDestroyed":
                case "UnitBounced":
                    badge = $"[{actor}] {evt.eventType.ToUpperInvariant()}\n{cardName}";
                    break;
                default:
                    badge = $"[{actor}] {evt.eventType}\n{cardName}";
                    break;
            }

            HistoryTileEntry entry = new HistoryTileEntry
            {
                eventRecord = evt,
                summaryTitle = badge,
                summaryDetail = !string.IsNullOrWhiteSpace(evt.text) ? evt.text : cardName
            };

            _entries.Insert(0, entry);
            while (_entries.Count > MaxHistoryEntries)
            {
                _entries.RemoveAt(_entries.Count - 1);
            }

            RefreshTileVisuals();
            return true;
        }

        public void ClearLog()
        {
            EnsureBuilt();
            _entries.Clear();
            RefreshTileVisuals();
        }

        private static bool IsLoggableEvent(string eventType)
        {
            switch (eventType)
            {
                case "CardPlayed":
                case "UnitPlaced":
                case "UnitSummoned":
                case "SpellCast":
                case "ShieldPlaced":
                case "AttackDeclared":
                case "CombatResolved":
                case "DamageDealt":
                case "ShieldRevealed":
                case "ShieldBroken":
                case "CardDestroyed":
                case "UnitDestroyed":
                case "UnitBounced":
                    return true;
                default:
                    return false;
            }
        }

        private void RefreshTileVisuals()
        {
            if (_propBlock == null)
            {
                _propBlock = new MaterialPropertyBlock();
            }

            for (int i = 0; i < _tileTransforms.Count; i++)
            {
                Transform tileTr = _tileTransforms[i];
                if (tileTr == null) continue;

                if (i < _entries.Count)
                {
                    HistoryTileEntry e = _entries[i];
                    tileTr.gameObject.SetActive(true);
                    if (i < _tileLabels.Count && _tileLabels[i] != null)
                    {
                        _tileLabels[i].text = e.summaryTitle;
                    }
                    if (i < _tileRenderers.Count && _tileRenderers[i] != null)
                    {
                        Color tileCol = e.eventRecord.seat == 0
                            ? new Color(0.20f, 0.38f, 0.62f, 1f)
                            : new Color(0.62f, 0.22f, 0.28f, 1f);
                        _tileRenderers[i].GetPropertyBlock(_propBlock);
                        _propBlock.SetColor("_BaseColor", tileCol);
                        _propBlock.SetColor("_Color", tileCol);
                        _tileRenderers[i].SetPropertyBlock(_propBlock);
                    }
                }
                else
                {
                    tileTr.gameObject.SetActive(false);
                }
            }
        }

        /// <summary>
        /// Checks if a raycast hit one of the compact history tiles and shows its card details in CardInspectorOverlay.
        /// </summary>
        public bool TryHoverOrClickTile(Transform hitTransform)
        {
            EnsureBuilt();
            if (hitTransform == null)
            {
                return false;
            }

            for (int i = 0; i < _tileTransforms.Count && i < _entries.Count; i++)
            {
                Transform t = _tileTransforms[i];
                if (t != null && t.gameObject.activeSelf && (hitTransform == t || hitTransform.IsChildOf(t)))
                {
                    HistoryTileEntry entry = _entries[i];
                    if (inspectorOverlay != null && inspectorOverlay.PreviewCardActor != null)
                    {
                        VisualEventRecord evt = entry.eventRecord;
                        inspectorOverlay.PreviewCardActor.ConfigureCard(
                            evt.cardUid != 0 ? evt.cardUid : -500 - i,
                            !string.IsNullOrEmpty(evt.cardSpecId) ? evt.cardSpecId : "H3W-AL-001",
                            evt.seat,
                            !string.IsNullOrEmpty(evt.title) ? evt.title : evt.eventType,
                            !string.IsNullOrEmpty(evt.cardType) ? evt.cardType : "Action",
                            Mathf.Max(1, evt.cost),
                            !string.IsNullOrEmpty(evt.triggerText) ? evt.triggerText : entry.summaryDetail,
                            Mathf.Max(1, evt.cost),
                            Mathf.Max(1, evt.cost + 1),
                            false,
                            "history",
                            false);
                        inspectorOverlay.InspectCard(inspectorOverlay.PreviewCardActor);
                    }
                    return true;
                }
            }
            return false;
        }
    }
}

