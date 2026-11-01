using System;
using System.Collections.Generic;
using Heav3ns.Visuals;
using TMPro;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>
    /// Multi-layered AudioSource pool with pitch randomization (+/- 4%) for card draw snaps,
    /// heavy cardboard board thuds, shield shatters, and button clicks, plus 2 clickable
    /// interactive tabletop props on the corners of EnchantedTableStage (an Enchanted Pocket Watch
    /// and a Wonderland Teacup) that react with SecondOrderSpring physics and particle bursts when clicked.
    /// </summary>
    [DisallowMultipleComponent]
    public class TabletopAudioAndProps : MonoBehaviour
    {
        public enum SoundEffectType
        {
            CardDrawSnap = 0,
            CardboardBoardThud = 1,
            ShieldShatter = 2,
            ButtonClick = 3,
            PropChime = 4
        }

        [Header("Multi-Layered Audio Pool")]
        [SerializeField] private int audioPoolSize = 6;
        [SerializeField] private float pitchVariance = 0.04f; // +/- 4%
        [SerializeField] private List<AudioSource> audioSources = new List<AudioSource>();

        [Header("Interactive Corner Props")]
        [SerializeField] private Transform pocketWatchProp;
        [SerializeField] private Transform teacupProp;
        [SerializeField] private SecondOrderSpring pocketWatchSpring;
        [SerializeField] private SecondOrderSpring teacupSpring;
        [SerializeField] private ParticleSystem pocketWatchParticles;
        [SerializeField] private ParticleSystem teacupParticles;

        private AudioClip _drawSnapClip;
        private AudioClip _boardThudClip;
        private AudioClip _shieldShatterClip;
        private AudioClip _buttonClickClip;
        private AudioClip _propChimeClip;
        private int _poolCursor;
        private int _pocketWatchClickCount;
        private int _teacupClickCount;

        public Transform PocketWatchProp => pocketWatchProp;
        public Transform TeacupProp => teacupProp;
        public int PocketWatchClickCount => _pocketWatchClickCount;
        public int TeacupClickCount => _teacupClickCount;
        public IReadOnlyList<AudioSource> AudioPool => audioSources;

        private void Awake()
        {
            EnsureInitialized();
        }

        public void EnsureInitialized()
        {
            EnsureAudioPool();
            EnsureSynthesizedClips();
            EnsureCornerProps();
        }

        private void EnsureAudioPool()
        {
            audioSources.RemoveAll(a => a == null);
            while (audioSources.Count < audioPoolSize)
            {
                GameObject srcGo = new GameObject($"TabletopAudioLayer_{audioSources.Count}");
                srcGo.transform.SetParent(transform, false);
                AudioSource src = srcGo.AddComponent<AudioSource>();
                src.playOnAwake = false;
                src.spatialBlend = 0.25f;
                audioSources.Add(src);
            }
        }

        private void EnsureSynthesizedClips()
        {
            if (_drawSnapClip == null)
            {
                _drawSnapClip = SynthesizeClip("CardDrawSnap", 0.09f, 520f, 1180f, 38f);
            }
            if (_boardThudClip == null)
            {
                _boardThudClip = SynthesizeClip("CardboardBoardThud", 0.16f, 130f, 58f, 22f);
            }
            if (_shieldShatterClip == null)
            {
                _shieldShatterClip = SynthesizeClip("ShieldShatter", 0.34f, 880f, 220f, 11f, true);
            }
            if (_buttonClickClip == null)
            {
                _buttonClickClip = SynthesizeClip("ButtonClick", 0.08f, 680f, 420f, 44f);
            }
            if (_propChimeClip == null)
            {
                _propChimeClip = SynthesizeClip("PropChime", 0.28f, 940f, 1420f, 14f);
            }
        }

        private static AudioClip SynthesizeClip(
            string clipName,
            float durationSeconds,
            float freqStart,
            float freqEnd,
            float decayRate,
            bool addNoise = false)
        {
            const int sampleRate = 22050;
            int samples = Mathf.Max(64, Mathf.RoundToInt(durationSeconds * sampleRate));
            float[] data = new float[samples];
            uint rng = 0x13579BDFU;

            for (int i = 0; i < samples; i++)
            {
                float t = (float)i / sampleRate;
                float norm = (float)i / samples;
                float freq = Mathf.Lerp(freqStart, freqEnd, norm);
                float env = Mathf.Exp(-decayRate * t);
                float wave = Mathf.Sin(2f * Mathf.PI * freq * t);
                if (addNoise)
                {
                    rng = rng * 1664525U + 1013904223U;
                    float noise = ((rng & 0xFFFF) / 32768f) - 1f;
                    wave = (wave * 0.55f) + (noise * 0.45f);
                }
                data[i] = Mathf.Clamp(wave * env * 0.45f, -1f, 1f);
            }

            AudioClip clip = AudioClip.Create(clipName, samples, 1, sampleRate, false);
            clip.SetData(data, 0);
            return clip;
        }

        public void EnsureCornerProps()
        {
            GameObject stage = GameObject.Find("EnchantedTableStage");
            Transform parent = stage != null ? stage.transform : transform;

            if (pocketWatchProp == null)
            {
                Transform existingWatch = parent.Find("EnchantedPocketWatchProp");
                if (existingWatch != null)
                {
                    pocketWatchProp = existingWatch;
                    pocketWatchSpring = existingWatch.GetComponent<SecondOrderSpring>();
                    pocketWatchParticles = existingWatch.GetComponentInChildren<ParticleSystem>();
                }
                else
                {
                    pocketWatchProp = CreateCornerProp(
                        parent,
                        "EnchantedPocketWatchProp",
                        new Vector3(-7.4f, 0.25f, 4.4f),
                        new Color(0.92f, 0.76f, 0.28f, 1f),
                        "Pocket Watch",
                        out pocketWatchSpring,
                        out pocketWatchParticles);
                }
            }

            if (teacupProp == null)
            {
                Transform existingCup = parent.Find("WonderlandTeacupProp");
                if (existingCup != null)
                {
                    teacupProp = existingCup;
                    teacupSpring = existingCup.GetComponent<SecondOrderSpring>();
                    teacupParticles = existingCup.GetComponentInChildren<ParticleSystem>();
                }
                else
                {
                    teacupProp = CreateCornerProp(
                        parent,
                        "WonderlandTeacupProp",
                        new Vector3(7.4f, 0.25f, 4.4f),
                        new Color(0.52f, 0.86f, 0.96f, 1f),
                        "Mad Teacup",
                        out teacupSpring,
                        out teacupParticles);
                }
            }
        }

        private static Transform CreateCornerProp(
            Transform parent,
            string name,
            Vector3 localPos,
            Color tint,
            string labelText,
            out SecondOrderSpring spring,
            out ParticleSystem particles)
        {
            GameObject propGo = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
            propGo.name = name;
            propGo.transform.SetParent(parent, false);
            propGo.transform.localPosition = localPos;
            propGo.transform.localScale = new Vector3(0.95f, 0.22f, 0.95f);

            Renderer rend = propGo.GetComponent<Renderer>();
            if (rend != null)
            {
                MaterialPropertyBlock block = new MaterialPropertyBlock();
                rend.GetPropertyBlock(block);
                block.SetColor("_BaseColor", tint);
                block.SetColor("_Color", tint);
                rend.SetPropertyBlock(block);
            }

            spring = propGo.AddComponent<SecondOrderSpring>();
            spring.SetTarget(localPos, Quaternion.identity, propGo.transform.localScale, true);

            // Label on top of prop
            GameObject labelGo = new GameObject("PropLabel");
            labelGo.transform.SetParent(propGo.transform, false);
            labelGo.transform.localPosition = new Vector3(0f, 1.25f, 0f);
            labelGo.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            TextMeshPro tmp = labelGo.AddComponent<TextMeshPro>();
            tmp.text = labelText;
            tmp.fontSize = 1.6f;
            tmp.color = Color.white;
            tmp.alignment = TextAlignmentOptions.Center;

            // Particle emitter child
            GameObject fxGo = new GameObject("PropParticles");
            fxGo.transform.SetParent(propGo.transform, false);
            fxGo.transform.localPosition = Vector3.up * 0.6f;
            particles = fxGo.AddComponent<ParticleSystem>();
            var main = particles.main;
            main.playOnAwake = false;
            main.duration = 0.35f;
            main.loop = false;
            main.startLifetime = 0.45f;
            main.startSpeed = 2.2f;
            main.startSize = 0.14f;
            main.startColor = tint;
            var emission = particles.emission;
            emission.enabled = false;
            particles.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);

            return propGo.transform;
        }

        /// <summary>
        /// Plays a synthesized tabletop sound effect on the next pooled AudioSource with +/- 4% pitch randomization.
        /// Returns the randomized pitch used.
        /// </summary>
        public float PlaySound(SoundEffectType type, float volume = 0.8f)
        {
            EnsureInitialized();
            if (audioSources.Count == 0)
            {
                return 1f;
            }

            AudioSource src = audioSources[_poolCursor % audioSources.Count];
            _poolCursor++;

            float randomPitch = 1f + UnityEngine.Random.Range(-pitchVariance, pitchVariance);
            src.pitch = randomPitch;
            src.volume = Mathf.Clamp01(volume);

            AudioClip clip = _buttonClickClip;
            switch (type)
            {
                case SoundEffectType.CardDrawSnap:
                    clip = _drawSnapClip;
                    break;
                case SoundEffectType.CardboardBoardThud:
                    clip = _boardThudClip;
                    break;
                case SoundEffectType.ShieldShatter:
                    clip = _shieldShatterClip;
                    break;
                case SoundEffectType.ButtonClick:
                    clip = _buttonClickClip;
                    break;
                case SoundEffectType.PropChime:
                    clip = _propChimeClip;
                    break;
            }

            if (clip != null)
            {
                src.PlayOneShot(clip, src.volume);
            }
            return randomPitch;
        }

        private void Update()
        {
            if (!UnityEngine.Input.GetMouseButtonDown(0))
            {
                return;
            }
            Camera cam = Camera.main;
            if (cam == null)
            {
                return;
            }
            Ray ray = cam.ScreenPointToRay(UnityEngine.Input.mousePosition);
            if (Physics.Raycast(ray, out RaycastHit hit, 100f))
            {
                TryInteractWithProp(hit.transform);
            }
        }

        public bool TryInteractWithProp(Transform hitTransform)
        {
            EnsureInitialized();
            if (hitTransform == null)
            {
                return false;
            }

            if (pocketWatchProp != null && (hitTransform == pocketWatchProp || hitTransform.IsChildOf(pocketWatchProp)))
            {
                _pocketWatchClickCount++;
                if (pocketWatchSpring != null)
                {
                    pocketWatchSpring.TriggerImpactSquashPunch();
                    pocketWatchSpring.AddImpulse(new Vector3(0f, 3.8f, 0f));
                }
                if (pocketWatchParticles != null)
                {
                    pocketWatchParticles.Emit(14);
                }
                PlaySound(SoundEffectType.PropChime);
                return true;
            }

            if (teacupProp != null && (hitTransform == teacupProp || hitTransform.IsChildOf(teacupProp)))
            {
                _teacupClickCount++;
                if (teacupSpring != null)
                {
                    teacupSpring.TriggerImpactSquashPunch();
                    teacupSpring.AddImpulse(new Vector3(0f, 3.8f, 0f));
                }
                if (teacupParticles != null)
                {
                    teacupParticles.Emit(14);
                }
                PlaySound(SoundEffectType.PropChime);
                return true;
            }

            return false;
        }
    }
}

