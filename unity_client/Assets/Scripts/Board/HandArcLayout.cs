using System.Collections.Generic;
using Heav3ns.Visuals;
using UnityEngine;

namespace Heav3ns.Board
{
    /// <summary>
    /// Deterministic spatial layout controller for AliceHandAnchor and WhiteRabbitHandAnchor.
    /// Arranges cards along a quadratic Bezier fan curve with subtle Z-axis fan rotation,
    /// and applies Hover Magnification (elevating Y/Z toward camera, flattening rotation to 0 deg,
    /// scaling by 1.35x, and parting neighboring cards horizontally).
    /// </summary>
    [DisallowMultipleComponent]
    public class HandArcLayout : MonoBehaviour
    {
        [Header("Hand Owner Configuration")]
        [SerializeField] private int ownerSeat = 0;
        [SerializeField] private bool maskCardsAsFaceDown = false;

        [Header("Quadratic Bezier Fan Curve")]
        [SerializeField] private float maxArcWidth = 6.2f;
        [SerializeField] private float cardSpacing = 1.15f;
        [SerializeField] private float arcApexDepthOffset = 0.42f;
        [SerializeField] private float maxFanAngleDegrees = 12f;
        [SerializeField] private float verticalStepPerCard = 0.012f;

        [Header("Hover Magnification (AliceHandAnchor)")]
        [SerializeField] private float hoverScaleMultiplier = 1.35f;
        [SerializeField] private float hoverElevationY = 0.55f;
        [SerializeField] private float hoverCameraOffsetZ = 0.65f;
        [SerializeField] private float neighborPartingOffset = 0.58f;

        private readonly List<CardVisual> _cards = new List<CardVisual>();
        private CardVisual _hoveredCard;
        private CardVisual _draggedCard;

        public int OwnerSeat
        {
            get => ownerSeat;
            set => ownerSeat = value;
        }

        public bool MaskCardsAsFaceDown
        {
            get => maskCardsAsFaceDown;
            set => maskCardsAsFaceDown = value;
        }

        public IReadOnlyList<CardVisual> Cards => _cards;
        public CardVisual HoveredCard => _hoveredCard;

        public void ConfigureHandZone(int seat, bool faceDown)
        {
            ownerSeat = seat;
            maskCardsAsFaceDown = faceDown;
            if (seat == 1 && arcApexDepthOffset > 0f)
            {
                arcApexDepthOffset = -Mathf.Abs(arcApexDepthOffset);
            }
        }

        public void AddCard(CardVisual card, bool snapImmediately = false)
        {
            if (card == null)
            {
                return;
            }
            if (!_cards.Contains(card))
            {
                _cards.Add(card);
            }
            card.transform.SetParent(transform, true);
            card.SetZone("hand");
            card.SetFaceDown(maskCardsAsFaceDown || ownerSeat != 0);
            RebuildLayout(snapImmediately);
        }

        public bool RemoveCard(CardVisual card)
        {
            if (card == null)
            {
                return false;
            }
            if (_hoveredCard == card)
            {
                _hoveredCard = null;
            }
            if (_draggedCard == card)
            {
                _draggedCard = null;
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
                    if (Application.isPlaying)
                    {
                        Destroy(c.gameObject);
                    }
                    else
                    {
                        DestroyImmediate(c.gameObject);
                    }
                }
            }
            if (destroyGameObjects)
            {
                for (int i = transform.childCount - 1; i >= 0; i--)
                {
                    Transform child = transform.GetChild(i);
                    if (child != null && child.GetComponent<CardVisual>() != null)
                    {
                        if (Application.isPlaying)
                        {
                            Destroy(child.gameObject);
                        }
                        else
                        {
                            DestroyImmediate(child.gameObject);
                        }
                    }
                }
            }
            _cards.Clear();
            _hoveredCard = null;
            _draggedCard = null;
        }

        public void SetHoveredCard(CardVisual card)
        {
            if (ownerSeat != 0)
            {
                // Hover magnification only reveals Alice's hand
                _hoveredCard = null;
                return;
            }

            if (_hoveredCard == card)
            {
                return;
            }

            _hoveredCard = (card != null && _cards.Contains(card)) ? card : null;
            RebuildLayout(false);
        }

        public void SetDraggedCard(CardVisual card)
        {
            _draggedCard = card;
            RebuildLayout(false);
        }

        /// <summary>
        /// Evaluates a quadratic Bezier point B(t) = (1-t)^2 P0 + 2(1-t)t P1 + t^2 P2.
        /// </summary>
        public static Vector3 EvaluateQuadraticBezier(Vector3 p0, Vector3 p1, Vector3 p2, float t)
        {
            float u = 1f - t;
            return (u * u * p0) + (2f * u * t * p1) + (t * t * p2);
        }

        public void RebuildLayout(bool snapImmediately = false)
        {
            _cards.RemoveAll(c => c == null);
            int count = _cards.Count;
            if (count == 0)
            {
                return;
            }

            int hoveredIndex = _hoveredCard != null ? _cards.IndexOf(_hoveredCard) : -1;
            float totalWidth = Mathf.Min(maxArcWidth, Mathf.Max(0f, (count - 1) * cardSpacing));
            float halfWidth = totalWidth * 0.5f;

            Vector3 p0 = new Vector3(-halfWidth, 0f, 0f);
            Vector3 p1 = new Vector3(0f, 0f, arcApexDepthOffset);
            Vector3 p2 = new Vector3(halfWidth, 0f, 0f);

            for (int i = 0; i < count; i++)
            {
                CardVisual card = _cards[i];
                if (card == null || card == _draggedCard)
                {
                    continue;
                }

                card.SetFaceDown(maskCardsAsFaceDown || ownerSeat != 0);

                float t = count == 1 ? 0.5f : (float)i / (count - 1);
                Vector3 curvePos = EvaluateQuadraticBezier(p0, p1, p2, t);
                curvePos.y += i * verticalStepPerCard;

                // Subtle Z-axis fan rotation across the Bezier arc
                float normalizedSigned = (t - 0.5f) * 2f; // -1 .. +1
                float zFanDegrees = -normalizedSigned * maxFanAngleDegrees * (ownerSeat == 0 ? 1f : -1f);
                Quaternion targetRot = Quaternion.Euler(0f, ownerSeat == 0 ? 0f : 180f, zFanDegrees);
                Vector3 targetScale = Vector3.one;

                if (hoveredIndex >= 0 && ownerSeat == 0)
                {
                    if (i == hoveredIndex)
                    {
                        // Elevate Y/Z toward camera, flatten rotation to 0 deg, scale by 1.35x
                        curvePos.y += hoverElevationY;
                        curvePos.z += hoverCameraOffsetZ;
                        targetRot = Quaternion.Euler(0f, 0f, 0f);
                        targetScale = Vector3.one * hoverScaleMultiplier;
                    }
                    else
                    {
                        // Part neighboring cards horizontally so full card text is unobstructed
                        int delta = i - hoveredIndex;
                        float falloff = 1f / Mathf.Abs(delta);
                        curvePos.x += Mathf.Sign(delta) * neighborPartingOffset * falloff;
                    }
                }

                card.Spring.SetTarget(curvePos, targetRot, targetScale, snapImmediately);
            }
        }
    }
}

