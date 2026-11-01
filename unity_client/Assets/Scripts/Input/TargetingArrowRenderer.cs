using System.Collections.Generic;
using UnityEngine;

namespace Heav3ns.Input
{
    /// <summary>
    /// Renders a curved 3D Bezier arc of animated chevron meshes from a source card
    /// to the cursor or snapped target when playing a targeted card or commanding a unit in AliceSequenceRow.
    /// </summary>
    [DisallowMultipleComponent]
    public class TargetingArrowRenderer : MonoBehaviour
    {
        [Header("Bezier Chevron Arc Settings")]
        [SerializeField] private int chevronCount = 14;
        [SerializeField] private float arcApexHeight = 2.1f;
        [SerializeField] private float chevronPulseSpeed = 6.0f;
        [SerializeField] private Color normalArrowColor = new Color(0.35f, 0.88f, 1.0f, 1.0f);
        [SerializeField] private Color snappedTargetColor = new Color(1.0f, 0.42f, 0.25f, 1.0f);

        private readonly List<Transform> _chevrons = new List<Transform>();
        private readonly List<Renderer> _chevronRenderers = new List<Renderer>();
        private Transform _reticleHead;
        private Renderer _reticleRenderer;
        private MaterialPropertyBlock _propBlock;
        private bool _isActive;
        private Vector3 _startWorldPos;
        private Vector3 _endWorldPos;
        private bool _isSnappedToValidTarget;
        private float _animTime;

        public bool IsActive => _isActive;
        public Vector3 StartWorldPos => _startWorldPos;
        public Vector3 EndWorldPos => _endWorldPos;
        public bool IsSnappedToValidTarget => _isSnappedToValidTarget;
        public int ChevronCount => _chevrons.Count;

        private void Awake()
        {
            EnsureChevronPool();
            HideArrow();
        }

        public void EnsureChevronPool()
        {
            if (_chevrons.Count >= chevronCount && _reticleHead != null)
            {
                return;
            }

            for (int i = _chevrons.Count; i < chevronCount; i++)
            {
                GameObject chev = GameObject.CreatePrimitive(PrimitiveType.Cube);
                chev.name = $"Chevron_{i:D2}";
                Collider col = chev.GetComponent<Collider>();
                if (col != null)
                {
                    if (Application.isPlaying) Destroy(col);
                    else DestroyImmediate(col);
                }
                chev.transform.SetParent(transform, false);
                chev.transform.localScale = new Vector3(0.24f, 0.04f, 0.16f);
                _chevrons.Add(chev.transform);
                _chevronRenderers.Add(chev.GetComponent<Renderer>());
            }

            if (_reticleHead == null)
            {
                GameObject head = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
                head.name = "TargetReticleHead";
                Collider col = head.GetComponent<Collider>();
                if (col != null)
                {
                    if (Application.isPlaying) Destroy(col);
                    else DestroyImmediate(col);
                }
                head.transform.SetParent(transform, false);
                head.transform.localScale = new Vector3(0.52f, 0.02f, 0.52f);
                _reticleHead = head.transform;
                _reticleRenderer = head.GetComponent<Renderer>();
            }
        }

        public void RenderBezierArrow(Vector3 startWorld, Vector3 endWorld, bool snappedToValidTarget)
        {
            EnsureChevronPool();
            _isActive = true;
            _startWorldPos = startWorld;
            _endWorldPos = endWorld;
            _isSnappedToValidTarget = snappedToValidTarget;

            SetVisualsActive(true);
            UpdateArcGeometry(Time.deltaTime);
        }

        public void HideArrow()
        {
            _isActive = false;
            _isSnappedToValidTarget = false;
            SetVisualsActive(false);
        }

        private void SetVisualsActive(bool active)
        {
            for (int i = 0; i < _chevrons.Count; i++)
            {
                if (_chevrons[i] != null)
                {
                    _chevrons[i].gameObject.SetActive(active);
                }
            }
            if (_reticleHead != null)
            {
                _reticleHead.gameObject.SetActive(active);
            }
        }

        private void Update()
        {
            if (_isActive)
            {
                UpdateArcGeometry(Time.deltaTime);
            }
        }

        public void UpdateArcGeometry(float deltaTime)
        {
            if (!_isActive)
            {
                return;
            }

            _animTime += deltaTime * chevronPulseSpeed;
            Vector3 midPoint = (_startWorldPos + _endWorldPos) * 0.5f;
            float distance = Vector3.Distance(_startWorldPos, _endWorldPos);
            Vector3 controlPoint = midPoint + Vector3.up * (arcApexHeight + distance * 0.18f);

            if (_propBlock == null)
            {
                _propBlock = new MaterialPropertyBlock();
            }

            Color activeColor = _isSnappedToValidTarget ? snappedTargetColor : normalArrowColor;

            int count = _chevrons.Count;
            for (int i = 0; i < count; i++)
            {
                Transform chev = _chevrons[i];
                if (chev == null) continue;

                // Animated marching offset along Bezier curve
                float rawT = ((float)i / count) + ((_animTime * 0.08f) % (1f / count));
                float t = Mathf.Clamp01(rawT);
                float nextT = Mathf.Clamp01(t + 0.04f);

                Vector3 pos = EvaluateBezier(_startWorldPos, controlPoint, _endWorldPos, t);
                Vector3 nextPos = EvaluateBezier(_startWorldPos, controlPoint, _endWorldPos, nextT);

                chev.position = pos;
                Vector3 tangent = nextPos - pos;
                if (tangent.sqrMagnitude > 0.0001f)
                {
                    chev.rotation = Quaternion.LookRotation(tangent.normalized, Vector3.up);
                }

                float scalePulse = 1f + 0.18f * Mathf.Sin(_animTime - i * 0.55f);
                chev.localScale = new Vector3(0.24f * scalePulse, 0.04f, 0.18f * scalePulse);

                Renderer rend = _chevronRenderers[i];
                if (rend != null)
                {
                    rend.GetPropertyBlock(_propBlock);
                    _propBlock.SetColor("_BaseColor", activeColor);
                    _propBlock.SetColor("_Color", activeColor);
                    rend.SetPropertyBlock(_propBlock);
                }
            }

            if (_reticleHead != null)
            {
                _reticleHead.position = _endWorldPos + Vector3.up * 0.04f;
                _reticleHead.rotation = Quaternion.Euler(0f, _animTime * 35f, 0f);
                if (_reticleRenderer != null)
                {
                    _reticleRenderer.GetPropertyBlock(_propBlock);
                    _propBlock.SetColor("_BaseColor", activeColor);
                    _propBlock.SetColor("_Color", activeColor);
                    _reticleRenderer.SetPropertyBlock(_propBlock);
                }
            }
        }

        public static Vector3 EvaluateBezier(Vector3 p0, Vector3 p1, Vector3 p2, float t)
        {
            float u = 1f - t;
            return (u * u * p0) + (2f * u * t * p1) + (t * t * p2);
        }
    }
}

