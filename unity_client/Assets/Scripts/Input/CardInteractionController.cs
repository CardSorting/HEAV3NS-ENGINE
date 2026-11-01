using System;
using Heav3ns.Board;
using Heav3ns.UI;
using Heav3ns.UnityBridge;
using Heav3ns.Visuals;
using UnityEngine;

namespace Heav3ns.Input
{
    /// <summary>
    /// Handles raycasting from Main Camera against CardVisual colliders and BoardSurface,
    /// drives the tactile 2.5D drag-and-drop state machine (Idle -> Hovering -> DraggingCard / AimingTarget -> ReleaseCommit / CancelReturn),
    /// renders the Bezier TargetingArrowRenderer, highlights valid targets with CardBorderGlow.shader,
    /// triggers the Phase 3 CardInspectorOverlay on right-click or 0.6s long-hover,
    /// and dispatches serialized commands to RustEngineBridge / VisualEventQueue.
    /// </summary>
    [DisallowMultipleComponent]
    public class CardInteractionController : MonoBehaviour
    {
        public enum DragState
        {
            Idle = 0,
            Hovering = 1,
            DraggingCard = 2,
            AimingTarget = 3,
            ReleaseCommit = 4,
            CancelReturn = 5
        }

        [Header("Scene References")]
        [SerializeField] private Camera mainCamera;
        [SerializeField] private Collider boardSurfaceCollider;
        [SerializeField] private HandArcLayout aliceHandLayout;
        [SerializeField] private BoardRowLayout aliceSequenceRow;
        [SerializeField] private BoardRowLayout aliceShieldRow;
        [SerializeField] private BoardRowLayout whiteRabbitSequenceRow;
        [SerializeField] private BoardRowLayout whiteRabbitShieldRow;
        [SerializeField] private TargetingArrowRenderer targetingArrow;
        [SerializeField] private VisualEventQueue eventQueue;
        [SerializeField] private CardInspectorOverlay inspectorOverlay;
        [SerializeField] private ActionHistoryLog actionHistoryLog;

        [Header("Interaction Tuning")]
        [SerializeField] private float dragHoverHeight = 0.85f;
        [SerializeField] private float playCommitZThreshold = -3.1f;
        [SerializeField] private bool interactionEnabled = true;

        private DragState _currentState = DragState.Idle;
        private CardVisual _hoveredCard;
        private CardVisual _activeCard;
        private CardVisual _snappedTargetCard;
        private Vector3 _currentBoardPoint;

        public DragState CurrentState => _currentState;
        public CardVisual HoveredCard => _hoveredCard;
        public CardVisual ActiveCard => _activeCard;
        public CardVisual SnappedTargetCard => _snappedTargetCard;
        public CardInspectorOverlay InspectorOverlay => inspectorOverlay;
        public ActionHistoryLog HistoryLog => actionHistoryLog;

        public bool InteractionEnabled
        {
            get => interactionEnabled;
            set
            {
                interactionEnabled = value;
                if (!interactionEnabled && (_currentState == DragState.DraggingCard || _currentState == DragState.AimingTarget))
                {
                    ExecuteCancelReturn();
                }
            }
        }

        public event Action<CardVisual, string, string> OnCardCommandCommitted;

        private void Awake()
        {
            EnsureReferences();
        }

        public void EnsureReferences()
        {
            if (mainCamera == null)
            {
                mainCamera = Camera.main != null ? Camera.main : FindAnyObjectByType<Camera>();
            }
            if (eventQueue == null)
            {
                eventQueue = FindAnyObjectByType<VisualEventQueue>();
            }
            if (inspectorOverlay == null)
            {
                inspectorOverlay = FindAnyObjectByType<CardInspectorOverlay>();
            }
            if (actionHistoryLog == null)
            {
                actionHistoryLog = FindAnyObjectByType<ActionHistoryLog>();
            }
            if (targetingArrow == null)
            {
                targetingArrow = GetComponentInChildren<TargetingArrowRenderer>();
                if (targetingArrow == null)
                {
                    targetingArrow = FindAnyObjectByType<TargetingArrowRenderer>();
                }
                if (targetingArrow == null)
                {
                    GameObject arrowGo = new GameObject("TargetingArrow");
                    arrowGo.transform.SetParent(transform, false);
                    targetingArrow = arrowGo.AddComponent<TargetingArrowRenderer>();
                }
            }

            GameObject stage = GameObject.Find("EnchantedTableStage");
            if (stage != null)
            {
                if (boardSurfaceCollider == null)
                {
                    Transform bs = stage.transform.Find("BoardSurface");
                    if (bs != null) boardSurfaceCollider = bs.GetComponent<Collider>();
                }
                if (aliceHandLayout == null)
                {
                    Transform ah = stage.transform.Find("AliceHandAnchor");
                    if (ah != null) aliceHandLayout = ah.GetComponent<HandArcLayout>();
                }
                if (aliceSequenceRow == null)
                {
                    Transform asr = stage.transform.Find("AliceSequenceRow");
                    if (asr != null) aliceSequenceRow = asr.GetComponent<BoardRowLayout>();
                }
                if (aliceShieldRow == null)
                {
                    Transform ash = stage.transform.Find("AliceShieldRow");
                    if (ash != null) aliceShieldRow = ash.GetComponent<BoardRowLayout>();
                }
                if (whiteRabbitSequenceRow == null)
                {
                    Transform wrs = stage.transform.Find("WhiteRabbitSequenceRow");
                    if (wrs != null) whiteRabbitSequenceRow = wrs.GetComponent<BoardRowLayout>();
                }
                if (whiteRabbitShieldRow == null)
                {
                    Transform wrsh = stage.transform.Find("WhiteRabbitShieldRow");
                    if (wrsh != null) whiteRabbitShieldRow = wrsh.GetComponent<BoardRowLayout>();
                }
            }
        }

        public void BindReferences(
            Camera cam,
            Collider boardSurface,
            HandArcLayout aliceHand,
            BoardRowLayout aliceSeq,
            BoardRowLayout aliceShield,
            BoardRowLayout rabbitSeq,
            BoardRowLayout rabbitShield,
            TargetingArrowRenderer arrow,
            VisualEventQueue queue,
            CardInspectorOverlay inspector = null,
            ActionHistoryLog history = null)
        {
            mainCamera = cam;
            boardSurfaceCollider = boardSurface;
            aliceHandLayout = aliceHand;
            aliceSequenceRow = aliceSeq;
            aliceShieldRow = aliceShield;
            whiteRabbitSequenceRow = rabbitSeq;
            whiteRabbitShieldRow = rabbitShield;
            targetingArrow = arrow;
            eventQueue = queue;
            if (inspector != null) inspectorOverlay = inspector;
            if (history != null) actionHistoryLog = history;
        }

        private void Update()
        {
            var router = FindAnyObjectByType<Heav3ns.UI.Navigation.HubNavigationRouter>();
            if (router != null && (router.IsTransitioning || router.IsPauseOpen)) return;
            if (!interactionEnabled || mainCamera == null)
            {
                return;
            }

            Vector3 mousePos = UnityEngine.Input.mousePosition;
            Ray ray = mainCamera.ScreenPointToRay(mousePos);
            ProcessPointerRay(
                ray,
                UnityEngine.Input.GetMouseButtonDown(0),
                UnityEngine.Input.GetMouseButton(0),
                UnityEngine.Input.GetMouseButtonUp(0),
                UnityEngine.Input.GetMouseButtonDown(1),
                Time.deltaTime);
        }

        /// <summary>
        /// Processes pointer raycast and state transitions. Can be driven either by live mouse input
        /// or deterministically by automated PlayMode / CLI simulations.
        /// </summary>
        public void ProcessPointerRay(
            Ray ray,
            bool pointerDown,
            bool pointerHeld,
            bool pointerUp,
            bool cancelPressed = false,
            float deltaTime = 0.016f)
        {
            EnsureReferences();

            if (cancelPressed && (_currentState == DragState.DraggingCard || _currentState == DragState.AimingTarget))
            {
                ExecuteCancelReturn();
                return;
            }

            CardVisual hitCard = RaycastCardVisual(ray);
            Vector3 boardPoint = RaycastBoardPoint(ray);
            _currentBoardPoint = boardPoint;

            // Right-click or 0.6s long-hover triggers CardInspectorOverlay when not dragging
            if (_currentState != DragState.DraggingCard && _currentState != DragState.AimingTarget)
            {
                if (inspectorOverlay != null)
                {
                    inspectorOverlay.TickHoverCandidate(hitCard, deltaTime, cancelPressed);
                }

                if (pointerDown && actionHistoryLog != null && Physics.Raycast(ray, out RaycastHit rawHit, 100f))
                {
                    actionHistoryLog.TryHoverOrClickTile(rawHit.transform);
                }
            }
            else if (inspectorOverlay != null && inspectorOverlay.IsShowing)
            {
                inspectorOverlay.HideInspection();
            }

            switch (_currentState)
            {
                case DragState.Idle:
                case DragState.Hovering:
                case DragState.ReleaseCommit:
                case DragState.CancelReturn:
                {
                    if (hitCard != null && hitCard.OwnerSeat == 0 && hitCard.CurrentZone == "hand")
                    {
                        _currentState = DragState.Hovering;
                        _hoveredCard = hitCard;
                        if (aliceHandLayout != null)
                        {
                            aliceHandLayout.SetHoveredCard(hitCard);
                        }

                        if (pointerDown && hitCard.IsPlayable)
                        {
                            BeginDragOrAim(hitCard, boardPoint);
                        }
                    }
                    else if (hitCard != null && hitCard.OwnerSeat == 0 && hitCard.CurrentZone == "field" && pointerDown)
                    {
                        // Commanding a stationed unit in AliceSequenceRow to aim/interact with a target
                        BeginAimFromFieldUnit(hitCard, boardPoint);
                    }
                    else
                    {
                        _currentState = DragState.Idle;
                        _hoveredCard = null;
                        if (aliceHandLayout != null)
                        {
                            aliceHandLayout.SetHoveredCard(null);
                        }
                    }
                    break;
                }

                case DragState.DraggingCard:
                {
                    if (_activeCard == null)
                    {
                        ExecuteCancelReturn();
                        break;
                    }

                    if (pointerHeld && !pointerUp)
                    {
                        UpdateDraggingCard(_activeCard, boardPoint, hitCard);
                    }
                    else if (pointerUp)
                    {
                        EvaluateReleaseCommitOrCancel(boardPoint, hitCard);
                    }
                    break;
                }

                case DragState.AimingTarget:
                {
                    if (_activeCard == null)
                    {
                        ExecuteCancelReturn();
                        break;
                    }

                    if (pointerHeld && !pointerUp)
                    {
                        UpdateAimingReticle(_activeCard, boardPoint, hitCard);
                    }
                    else if (pointerUp)
                    {
                        EvaluateAimRelease(boardPoint, hitCard);
                    }
                    break;
                }
            }
        }

        public void BeginDragOrAim(CardVisual card, Vector3 boardPoint)
        {
            if (card == null)
            {
                return;
            }

            ClearSnappedTargetHighlight();
            _activeCard = card;
            if (aliceHandLayout != null)
            {
                aliceHandLayout.SetHoveredCard(null);
                aliceHandLayout.SetDraggedCard(card);
            }

            bool requiresTarget = CardRequiresTarget(card);
            if (requiresTarget && boardPoint.z > playCommitZThreshold)
            {
                _currentState = DragState.AimingTarget;
                UpdateAimingReticle(card, boardPoint, null);
            }
            else
            {
                _currentState = DragState.DraggingCard;
                UpdateDraggingCard(card, boardPoint, null);
            }
        }

        public void BeginAimFromFieldUnit(CardVisual fieldCard, Vector3 boardPoint)
        {
            if (fieldCard == null)
            {
                return;
            }
            ClearSnappedTargetHighlight();
            _activeCard = fieldCard;
            _currentState = DragState.AimingTarget;
            UpdateAimingReticle(fieldCard, boardPoint, null);
        }

        private void UpdateDraggingCard(CardVisual card, Vector3 boardPoint, CardVisual hitCard)
        {
            if (CardRequiresTarget(card) && boardPoint.z > playCommitZThreshold + 0.6f)
            {
                _currentState = DragState.AimingTarget;
                UpdateAimingReticle(card, boardPoint, hitCard);
                return;
            }

            ClearSnappedTargetHighlight();
            if (targetingArrow != null)
            {
                targetingArrow.HideArrow();
            }

            Transform parent = card.transform.parent;
            Vector3 desiredWorld = new Vector3(boardPoint.x, boardPoint.y + dragHoverHeight, boardPoint.z);
            Vector3 targetLocal = parent != null ? parent.InverseTransformPoint(desiredWorld) : desiredWorld;

            card.Spring.SetTarget(targetLocal, Quaternion.identity, Vector3.one * 1.12f, false);

            // Show ghost-card insertion preview when hovering over AliceSequenceRow
            if (aliceSequenceRow != null && boardPoint.z > playCommitZThreshold)
            {
                aliceSequenceRow.UpdateGhostInsertionPreview(boardPoint);
            }
            else if (aliceSequenceRow != null)
            {
                aliceSequenceRow.ClearGhostInsertionPreview();
            }
        }

        private void UpdateAimingReticle(CardVisual sourceCard, Vector3 boardPoint, CardVisual hitCard)
        {
            if (aliceSequenceRow != null)
            {
                aliceSequenceRow.ClearGhostInsertionPreview();
            }

            bool validTarget = hitCard != null && hitCard != sourceCard;
            SetSnappedTargetCard(validTarget ? hitCard : null);

            Vector3 startWorld = sourceCard.transform.position + Vector3.up * 0.15f;
            Vector3 endWorld = validTarget
                ? hitCard.transform.position + Vector3.up * 0.15f
                : boardPoint + Vector3.up * 0.1f;

            if (targetingArrow != null)
            {
                targetingArrow.RenderBezierArrow(startWorld, endWorld, validTarget);
            }
        }

        private void SetSnappedTargetCard(CardVisual newTarget)
        {
            if (_snappedTargetCard == newTarget)
            {
                return;
            }
            if (_snappedTargetCard != null)
            {
                _snappedTargetCard.SetTargetedHighlight(false);
            }
            _snappedTargetCard = newTarget;
            if (_snappedTargetCard != null)
            {
                _snappedTargetCard.SetTargetedHighlight(true);
            }
        }

        private void ClearSnappedTargetHighlight()
        {
            if (_snappedTargetCard != null)
            {
                _snappedTargetCard.SetTargetedHighlight(false);
                _snappedTargetCard = null;
            }
        }

        private void EvaluateReleaseCommitOrCancel(Vector3 boardPoint, CardVisual hitCard)
        {
            int ghostSlot = (aliceSequenceRow != null && aliceSequenceRow.GhostInsertionIndex >= 0)
                ? aliceSequenceRow.GhostInsertionIndex
                : -1;
            if (aliceSequenceRow != null)
            {
                aliceSequenceRow.ClearGhostInsertionPreview();
            }
            if (targetingArrow != null)
            {
                targetingArrow.HideArrow();
            }

            if (_activeCard == null)
            {
                ExecuteCancelReturn();
                return;
            }

            // Check if dropped onto AliceShieldRow or AliceSequenceRow
            bool overShieldRow = aliceShieldRow != null && Vector3.Distance(boardPoint, aliceShieldRow.transform.position) < 2.2f;
            bool overSequenceZone = boardPoint.z > playCommitZThreshold;

            CardVisual target = _snappedTargetCard != null ? _snappedTargetCard : hitCard;
            ClearSnappedTargetHighlight();

            if (overShieldRow || overSequenceZone)
            {
                CommitPlayCard(_activeCard, target, overShieldRow, ghostSlot);
            }
            else
            {
                ExecuteCancelReturn();
            }
        }

        private void EvaluateAimRelease(Vector3 boardPoint, CardVisual hitCard)
        {
            if (targetingArrow != null)
            {
                targetingArrow.HideArrow();
            }
            if (aliceSequenceRow != null)
            {
                aliceSequenceRow.ClearGhostInsertionPreview();
            }

            CardVisual target = _snappedTargetCard != null ? _snappedTargetCard : (hitCard != _activeCard ? hitCard : null);
            ClearSnappedTargetHighlight();

            if (_activeCard != null && (target != null || boardPoint.z > playCommitZThreshold))
            {
                if (_activeCard.CurrentZone == "field")
                {
                    CommitFieldUnitAction(_activeCard, target);
                }
                else
                {
                    CommitPlayCard(_activeCard, target, false);
                }
            }
            else
            {
                ExecuteCancelReturn();
            }
        }

        /// <summary>
        /// Commands a stationed unit in AliceSequenceRow to resolve its sequence / interact with a target.
        /// </summary>
        public string CommitFieldUnitAction(CardVisual fieldCard, CardVisual targetCard = null)
        {
            if (fieldCard == null)
            {
                return string.Empty;
            }

            _currentState = DragState.ReleaseCommit;
            ClearSnappedTargetHighlight();

            int targetUid = (targetCard != null && targetCard != fieldCard && targetCard.CardUid > 0) ? targetCard.CardUid : 0;
            string commandJson = targetUid > 0
                ? $"{{\"command\":\"close\",\"target_uid\":{targetUid}}}"
                : "{\"command\":\"close\"}";

            string response = string.Empty;
            if (eventQueue != null)
            {
                response = eventQueue.SubmitCommandJson(commandJson);
            }

            _activeCard = null;
            OnCardCommandCommitted?.Invoke(fieldCard, commandJson, response);
            return response;
        }

        /// <summary>
        /// Commits a card play or targeted action, serializes the JSON command for RustEngineBridge,
        /// queues the resulting visual events into VisualEventQueue, and moves the card into its target row.
        /// </summary>
        public string CommitPlayCard(CardVisual card, CardVisual targetCard = null, bool placeInShieldRow = false, int slotIndex = -1)
        {
            if (card == null)
            {
                return string.Empty;
            }

            _currentState = DragState.ReleaseCommit;
            ClearSnappedTargetHighlight();

            int targetUid = (targetCard != null && targetCard != card) ? targetCard.CardUid : 0;
            int insertionSlot = slotIndex >= 0
                ? slotIndex
                : ((aliceSequenceRow != null && aliceSequenceRow.GhostInsertionIndex >= 0) ? aliceSequenceRow.GhostInsertionIndex : -1);
            string commandJson = targetUid > 0
                ? $"{{\"command\":\"place\",\"uid\":{card.CardUid},\"target_uid\":{targetUid}}}"
                : $"{{\"command\":\"place\",\"uid\":{card.CardUid}}}";

            string response = string.Empty;
            if (eventQueue != null)
            {
                response = eventQueue.SubmitCommandJson(commandJson);
            }

            bool rejected = !string.IsNullOrEmpty(response) && response.Contains("\"rejected\"");
            if (rejected && !placeInShieldRow)
            {
                ExecuteCancelReturn();
                return response;
            }

            if (aliceHandLayout != null)
            {
                aliceHandLayout.SetDraggedCard(null);
                aliceHandLayout.RemoveCard(card);
            }

            if (placeInShieldRow && aliceShieldRow != null)
            {
                aliceShieldRow.AddCard(card, -1, false, true);
            }
            else if (aliceSequenceRow != null)
            {
                aliceSequenceRow.AddCard(card, insertionSlot, false, true);
            }

            _activeCard = null;
            OnCardCommandCommitted?.Invoke(card, commandJson, response);
            return response;
        }

        public void ExecuteCancelReturn()
        {
            _currentState = DragState.CancelReturn;
            ClearSnappedTargetHighlight();
            if (targetingArrow != null)
            {
                targetingArrow.HideArrow();
            }
            if (aliceSequenceRow != null)
            {
                aliceSequenceRow.ClearGhostInsertionPreview();
            }
            if (aliceHandLayout != null)
            {
                aliceHandLayout.SetDraggedCard(null);
                aliceHandLayout.RebuildLayout(false);
            }
            _activeCard = null;
        }

        private static bool CardRequiresTarget(CardVisual card)
        {
            if (card == null || string.IsNullOrEmpty(card.RulesText))
            {
                return false;
            }
            string lower = card.RulesText.ToLowerInvariant();
            return lower.Contains("return a") || lower.Contains("return an") || lower.Contains("break") || lower.Contains("target");
        }

        private CardVisual RaycastCardVisual(Ray ray)
        {
            RaycastHit[] hits = Physics.RaycastAll(ray, 100f);
            float closestDist = float.MaxValue;
            CardVisual closestCard = null;

            for (int i = 0; i < hits.Length; i++)
            {
                CardVisual cv = hits[i].collider.GetComponentInParent<CardVisual>();
                if (cv != null && cv != _activeCard && hits[i].distance < closestDist)
                {
                    closestDist = hits[i].distance;
                    closestCard = cv;
                }
            }
            return closestCard;
        }

        private Vector3 RaycastBoardPoint(Ray ray)
        {
            if (boardSurfaceCollider != null && boardSurfaceCollider.Raycast(ray, out RaycastHit hit, 100f))
            {
                return hit.point;
            }

            Plane tablePlane = new Plane(Vector3.up, Vector3.zero);
            if (tablePlane.Raycast(ray, out float enter))
            {
                return ray.GetPoint(enter);
            }

            return Vector3.zero;
        }
    }
}
