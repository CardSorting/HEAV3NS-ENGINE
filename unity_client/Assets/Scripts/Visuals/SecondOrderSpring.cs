using System;
using UnityEngine;

namespace Heav3ns.Visuals
{
    /// <summary>
    /// Frame-rate independent second-order spring-damper solver for 2.5D card motion,
    /// velocity-driven pitch/roll tilt, dynamic shadow offset, and squash-and-stretch impact punches.
    /// </summary>
    [DisallowMultipleComponent]
    public class SecondOrderSpring : MonoBehaviour
    {
        [Header("Second-Order Spring Parameters")]
        [SerializeField] private float stiffness = 240f;
        [SerializeField] private float damping = 18f;
        [SerializeField] private float overshoot = 0.15f;

        [Header("Velocity-Driven 2.5D Tilt")]
        [SerializeField] private float maxTiltDegrees = 22f;
        [SerializeField] private float tiltVelocityScale = 3.2f;
        [SerializeField] private float tiltSpringSpeed = 16f;

        [Header("Dynamic Shadow")]
        [SerializeField] private Transform shadowTransform;
        [SerializeField] private Vector3 baseShadowOffset = new Vector3(0.06f, -0.03f, -0.06f);
        [SerializeField] private float shadowVelocityFactor = 0.018f;

        private Vector3 _targetPosition;
        private Vector3 _velocity;
        private Quaternion _targetBaseRotation = Quaternion.identity;
        private Vector3 _currentTiltEuler;
        private Vector3 _targetScale = Vector3.one;
        private Vector3 _scaleVelocity;

        // 2-frame impact punch state (1.15x X/Z, 0.85x Y)
        private int _impactPunchFramesRemaining;
        private Vector3 _impactPunchScale = new Vector3(1.15f, 0.85f, 1.15f);
        private bool _initialized;

        public float Stiffness
        {
            get => stiffness;
            set => stiffness = Mathf.Max(1f, value);
        }

        public float Damping
        {
            get => damping;
            set => damping = Mathf.Max(0.1f, value);
        }

        public float Overshoot
        {
            get => overshoot;
            set => overshoot = Mathf.Clamp(value, 0f, 1f);
        }

        public Vector3 TargetPosition => _targetPosition;
        public Quaternion TargetBaseRotation => _targetBaseRotation;
        public Vector3 TargetScale => _targetScale;
        public Vector3 Velocity => _velocity;
        public Vector2 PlanarVelocity => new Vector2(_velocity.x, _velocity.z);
        public float TiltPitchDegrees => _currentTiltEuler.x;
        public float TiltRollDegrees => _currentTiltEuler.z;
        public bool IsImpactPunchActive => _impactPunchFramesRemaining > 0;

        private void Awake()
        {
            EnsureInitialized();
        }

        public void EnsureInitialized()
        {
            if (_initialized)
            {
                return;
            }
            _initialized = true;
            _targetPosition = transform.localPosition;
            _targetBaseRotation = transform.localRotation;
            _targetScale = transform.localScale == Vector3.zero ? Vector3.one : transform.localScale;
        }

        public void ConfigureShadow(Transform shadow)
        {
            shadowTransform = shadow;
        }

        public void SetTarget(Vector3 localPosition, Quaternion localRotation, Vector3 localScale, bool snapImmediately = false)
        {
            EnsureInitialized();
            _targetPosition = localPosition;
            _targetBaseRotation = localRotation;
            _targetScale = localScale;

            if (snapImmediately)
            {
                transform.localPosition = localPosition;
                transform.localRotation = localRotation;
                transform.localScale = localScale;
                _velocity = Vector3.zero;
                _scaleVelocity = Vector3.zero;
                _currentTiltEuler = Vector3.zero;
                _impactPunchFramesRemaining = 0;
                UpdateShadowTransform(0f);
            }
        }

        public void SetTargetPosition(Vector3 localPosition, bool snapImmediately = false)
        {
            EnsureInitialized();
            _targetPosition = localPosition;
            if (snapImmediately)
            {
                transform.localPosition = localPosition;
                _velocity = Vector3.zero;
                UpdateShadowTransform(0f);
            }
        }

        public void SetTargetRotation(Quaternion localRotation, bool snapImmediately = false)
        {
            EnsureInitialized();
            _targetBaseRotation = localRotation;
            if (snapImmediately)
            {
                transform.localRotation = localRotation;
                _currentTiltEuler = Vector3.zero;
            }
        }

        public void SetTargetScale(Vector3 localScale, bool snapImmediately = false)
        {
            EnsureInitialized();
            _targetScale = localScale;
            if (snapImmediately)
            {
                transform.localScale = localScale;
                _scaleVelocity = Vector3.zero;
                _impactPunchFramesRemaining = 0;
            }
        }

        /// <summary>
        /// Triggers a 2-frame impact squash-and-stretch punch (1.15x X/Z, 0.85x Y) before springing back to TargetScale.
        /// </summary>
        public void TriggerImpactSquashPunch()
        {
            EnsureInitialized();
            _impactPunchFramesRemaining = 2;
            _impactPunchScale = new Vector3(
                _targetScale.x * 1.15f,
                _targetScale.y * 0.85f,
                _targetScale.z * 1.15f);
            transform.localScale = _impactPunchScale;
            _scaleVelocity = Vector3.zero;
        }

        public void AddImpulse(Vector3 velocityImpulse)
        {
            EnsureInitialized();
            _velocity += velocityImpulse;
        }

        private void Update()
        {
            Step(Time.deltaTime);
        }

        /// <summary>
        /// Frame-rate independent second-order spring-damper integration step.
        /// Sub-steps large deltaTimes for unconditional stability in both PlayMode and headless simulation.
        /// </summary>
        public void Step(float deltaTime)
        {
            EnsureInitialized();
            if (deltaTime <= 0f)
            {
                return;
            }

            float remaining = Mathf.Min(deltaTime, 0.25f);
            const float maxSubStep = 1f / 120f;

            // Effective damping reduced slightly by overshoot factor to produce crisp tabletop spring feel
            float effectiveStiffness = Mathf.Max(1f, stiffness);
            float effectiveDamping = Mathf.Max(0.5f, damping * (1f - Mathf.Clamp01(overshoot) * 0.45f));

            Vector3 pos = transform.localPosition;
            while (remaining > 0f)
            {
                float dt = Mathf.Min(remaining, maxSubStep);
                remaining -= dt;

                Vector3 displacement = pos - _targetPosition;
                Vector3 accel = (-effectiveStiffness * displacement) - (effectiveDamping * _velocity);
                _velocity += accel * dt;
                pos += _velocity * dt;
            }
            transform.localPosition = pos;

            // Velocity-driven 2.5D pitch (X) and roll (Z)
            // Moving right (+vx) rolls negative Z; moving forward (+vz / +vy) pitches positive X
            float vx = _velocity.x;
            float vy = Mathf.Abs(_velocity.z) > Mathf.Abs(_velocity.y) ? _velocity.z : _velocity.y;
            float desiredPitch = Mathf.Clamp(vy * tiltVelocityScale, -maxTiltDegrees, maxTiltDegrees);
            float desiredRoll = Mathf.Clamp(-vx * tiltVelocityScale, -maxTiltDegrees, maxTiltDegrees);

            float tiltLerp = 1f - Mathf.Exp(-tiltSpringSpeed * deltaTime);
            _currentTiltEuler.x = Mathf.Lerp(_currentTiltEuler.x, desiredPitch, tiltLerp);
            _currentTiltEuler.z = Mathf.Lerp(_currentTiltEuler.z, desiredRoll, tiltLerp);

            Quaternion tiltRotation = Quaternion.Euler(_currentTiltEuler.x, 0f, _currentTiltEuler.z);
            transform.localRotation = _targetBaseRotation * tiltRotation;

            // Squash & Stretch 2-frame impact punch followed by second-order spring back to _targetScale
            if (_impactPunchFramesRemaining > 0)
            {
                transform.localScale = _impactPunchScale;
                _impactPunchFramesRemaining--;
            }
            else
            {
                Vector3 currentScale = transform.localScale;
                float dtScale = Mathf.Min(deltaTime, 0.033f);
                Vector3 scaleDisp = currentScale - _targetScale;
                Vector3 scaleAccel = (-effectiveStiffness * 1.2f * scaleDisp) - (effectiveDamping * 1.1f * _scaleVelocity);
                _scaleVelocity += scaleAccel * dtScale;
                currentScale += _scaleVelocity * dtScale;
                transform.localScale = currentScale;
            }

            UpdateShadowTransform( pos.y );
        }

        private void UpdateShadowTransform(float currentHeight)
        {
            if (shadowTransform == null)
            {
                return;
            }

            float heightBoost = Mathf.Max(0f, currentHeight) * 0.12f;
            Vector3 velOffset = new Vector3(
                -_velocity.x * shadowVelocityFactor,
                0f,
                -_velocity.z * shadowVelocityFactor);
            shadowTransform.localPosition = baseShadowOffset + new Vector3(heightBoost, 0f, -heightBoost) + velOffset;
        }
    }
}

