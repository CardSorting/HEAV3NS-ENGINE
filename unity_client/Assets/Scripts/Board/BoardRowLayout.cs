using System.Collections.Generic;
using Heav3ns.Visuals;
using UnityEngine;

namespace Heav3ns.Board
{
    /// <summary>
    /// Deterministic spatial layout controller for AliceSequenceRow, AliceShieldRow,
    /// WhiteRabbitSequenceRow, and WhiteRabbitShieldRow.
    /// Manages slot spacing, ghost-card insertion previews while dragging a playable card over a row,
    /// subtle idle sine-wave breathing for stationed units/cards, and face-down orientation for unrevealed shields.
    /// </summary>
    [DisallowMultipleComponent]
    public class BoardRowLayout : MonoBehaviour
    {
        public enum RowKind
        {
            Sequence = 0,
            Shield = 1
        }

        [Header("Row Identity & Orientation")]
        [SerializeField] private int ownerSeat = 0;
        [SerializeField] private RowKind rowKind = RowKind.Sequence;
        [SerializeField] private bool enforceFaceDownShields = false;

        [Header("Slot Spacing")]
        [SerializeField] private float slotSpacingX = 1.62f;
        [SerializeField] private float shieldStackStepZ = 0.28f;
        [SerializeField] private float shieldStackStepX = 0.24f;
        [SerializeField] private int shieldsPerColumn = 3;

        [Header("Idle Sine-Wave Breathing")]
        [SerializeField] private bool enableIdleBreathing = true;
        [SerializeField] private float breathingAmplitudeY = 0.032f;
        [SerializeField] private float breathingFrequency = 2.1f;

        private readonly List<CardVisual> _cards = new List<CardVisual>();
        private GameObject _ghostPreviewObject;
        private int _ghostInsertionIndex = -1;
        private float _breathingClock;

        public int OwnerSeat => ownerSeat;
        public RowKind Kind => rowKind;
        public bool EnforceFaceDownShields => enforceFaceDownShields;
        public IReadOnlyList<CardVisual> Cards => _cards;
        public int GhostInsertionIndex => _ghostInsertionIndex;
        public bool HasGhostPreview => _ghostInsertionIndex >= 0 && _ghostPreviewObject != null && _ghostPreviewObject.activeSelf;

        private void Awake()
        {
            EnsureGhostPreviewObject();
        }

        public void ConfigureRow(int seat, RowKind kind, bool faceDownShields)
        {
            ownerSeat = seat;
            rowKind = kind;
            enforceFaceDownShields = faceDownShields;
            EnsureGhostPreviewObject();
        }

        private void EnsureGhostPreviewObject()
        {
            if (_ghostPreviewObject != null)
            {
                return;
            }
            Transform existing = transform.Find("GhostInsertionPreview");
            if (existing != null)
            {
                _ghostPreviewObject = existing.gameObject;
                _ghostPreviewObject.SetActive(false);
                return;
            }

            _ghostPreviewObject = GameObject.CreatePrimitive(PrimitiveType.Quad);
            _ghostPreviewObject.name = "GhostInsertionPreview";
            Collider col = _ghostPreviewObject.GetComponent<Collider>();
            if (col != null)
            {
                if (Application.isPlaying) Destroy(col);
                else DestroyImmediate(col);
            }
            _ghostPreviewObject.transform.SetParent(transform, false);
            _ghostPreviewObject.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            _ghostPreviewObject.transform.localScale = new Vector3(1.36f, 1.92f, 1f);

            Renderer rend = _ghostPreviewObject.GetComponent<Renderer>();
            if (rend != null)
            {
                MaterialPropertyBlock block = new MaterialPropertyBlock();
                rend.GetPropertyBlock(block);
                Color ghostColor = new Color(0.35f, 0.92f, 1.0f, 0.45f);
                block.SetColor("_BaseColor", ghostColor);
                block.SetColor("_Color", ghostColor);
                rend.SetPropertyBlock(block);
            }
            _ghostPreviewObject.SetActive(false);
        }

        public void AddCard(CardVisual card, int slotIndex = -1, bool snapImmediately = false, bool triggerImpactPunch = true)
        {
            if (card == null)
            {
                return;
            }

            if (!_cards.Contains(card))
            {
                if (slotIndex >= 0 && slotIndex <= _cards.Count)
                {
                    _cards.Insert(slotIndex, card);
                }
                else
                {
                    _cards.Add(card);
                }
            }

            card.transform.SetParent(transform, true);
            card.SetZone(rowKind == RowKind.Shield ? "shields" : "field");
            if (rowKind == RowKind.Shield && enforceFaceDownShields)
            {
                card.SetFaceDown(true);
            }
            else if (rowKind == RowKind.Sequence)
            {
                card.SetFaceDown(false);
            }

            RebuildLayout(snapImmediately);

            if (triggerImpactPunch && card.Spring != null)
            {
                card.Spring.TriggerImpactSquashPunch();
            }
        }

        public bool RemoveCard(CardVisual card)
        {
            if (card == null)
            {
                return false;
            }
            bool removed = _cards.Remove(card);
            if (removed)
            {
                RebuildLayout(false);
            }
            return removed;
        }

        public void ClearCards(bool destroyGameObjects = true)
        {
            for (int i = _cards.Count - 1; i >= 0; i--)
            {
                CardVisual c = _cards[i];
                if (c != null && destroyGameObjects)
                {
                    if (Application.isPlaying) Destroy(c.gameObject);
                    else DestroyImmediate(c.gameObject);
                }
            }
            if (destroyGameObjects)
            {
                for (int i = transform.childCount - 1; i >= 0; i--)
                {
                    Transform child = transform.GetChild(i);
                    if (child != null && child.GetComponent<CardVisual>() != null)
                    {
                        if (Application.isPlaying) Destroy(child.gameObject);
                        else DestroyImmediate(child.gameObject);
                    }
                }
            }
            _cards.Clear();
            ClearGhostInsertionPreview();
        }

        /// <summary>
        /// Shows a ghost-card insertion preview at the nearest slot for a world-space drag cursor position,
        /// parting existing cards around the previewed slot.
        /// </summary>
        public int UpdateGhostInsertionPreview(Vector3 worldDragPosition)
        {
            EnsureGhostPreviewObject();
            Vector3 localCursor = transform.InverseTransformPoint(worldDragPosition);
            int count = _cards.Count;

            int insertIdx = 0;
            if (count > 0)
            {
                float halfWidth = (count * slotSpacingX) * 0.5f;
                float normalized = (localCursor.x + halfWidth) / Mathf.Max(0.1f, slotSpacingX);
                insertIdx = Mathf.Clamp(Mathf.RoundToInt(normalized), 0, count);
            }

            _ghostInsertionIndex = insertIdx;
            if (_ghostPreviewObject != null)
            {
                _ghostPreviewObject.SetActive(true);
            }
            RebuildLayout(false);
            return _ghostInsertionIndex;
        }

        public void ClearGhostInsertionPreview()
        {
            if (_ghostInsertionIndex == -1 && (_ghostPreviewObject == null || !_ghostPreviewObject.activeSelf))
            {
                return;
            }
            _ghostInsertionIndex = -1;
            if (_ghostPreviewObject != null)
            {
                _ghostPreviewObject.SetActive(false);
            }
            RebuildLayout(false);
        }

        private void Update()
        {
            if (enableIdleBreathing && _cards.Count > 0)
            {
                TickIdleBreathing(Time.deltaTime);
            }
        }

        public void TickIdleBreathing(float deltaTime)
        {
            _breathingClock += deltaTime * breathingFrequency;
            RebuildLayout(false);
        }

        public Vector3 GetSlotLocalPosition(int index, int totalLogicalSlots)
        {
            if (rowKind == RowKind.Shield)
            {
                int col = index / Mathf.Max(1, shieldsPerColumn);
                int row = index % Mathf.Max(1, shieldsPerColumn);
                float dir = ownerSeat == 0 ? 1f : -1f;
                return new Vector3(
                    col * (1.28f * dir) + (row * shieldStackStepX * 0.15f),
                    row * 0.025f,
                    (row - 1f) * shieldStackStepZ * 3.2f);
            }

            float width = Mathf.Max(0, totalLogicalSlots - 1) * slotSpacingX;
            float startX = -width * 0.5f;
            return new Vector3(startX + index * slotSpacingX, 0f, 0f);
        }

        public void RebuildLayout(bool snapImmediately = false)
        {
            _cards.RemoveAll(c => c == null);
            int count = _cards.Count;
            bool hasGhost = _ghostInsertionIndex >= 0 && rowKind == RowKind.Sequence;
            int totalSlots = count + (hasGhost ? 1 : 0);

            if (hasGhost && _ghostPreviewObject != null)
            {
                int clampedGhost = Mathf.Clamp(_ghostInsertionIndex, 0, count);
                Vector3 ghostPos = GetSlotLocalPosition(clampedGhost, totalSlots);
                ghostPos.y = 0.015f;
                _ghostPreviewObject.transform.localPosition = ghostPos;
            }

            for (int i = 0; i < count; i++)
            {
                CardVisual card = _cards[i];
                if (card == null)
                {
                    continue;
                }

                if (rowKind == RowKind.Shield && enforceFaceDownShields)
                {
                    card.SetFaceDown(true);
                }

                int logicalSlot = (hasGhost && i >= _ghostInsertionIndex) ? i + 1 : i;
                Vector3 slotPos = GetSlotLocalPosition(logicalSlot, totalSlots);

                if (enableIdleBreathing)
                {
                    float phase = _breathingClock + (i * 0.85f) + (ownerSeat * 1.5f);
                    slotPos.y += Mathf.Sin(phase) * breathingAmplitudeY;
                }

                Quaternion slotRot = rowKind == RowKind.Shield
                    ? Quaternion.Euler(0f, ownerSeat == 0 ? 8f : 188f, 0f)
                    : Quaternion.Euler(0f, ownerSeat == 0 ? 0f : 180f, 0f);

                Vector3 slotScale = rowKind == RowKind.Shield
                    ? new Vector3(0.84f, 1f, 0.84f)
                    : Vector3.one;

                card.Spring.SetTarget(slotPos, slotRot, slotScale, snapImmediately);
            }
        }
    }
}

