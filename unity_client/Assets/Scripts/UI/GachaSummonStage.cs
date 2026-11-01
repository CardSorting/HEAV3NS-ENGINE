using System;
using System.Collections;
using System.Collections.Generic;
using Heav3ns.UI.Navigation;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>Owns one paid pack from purchase through reveal and recap.</summary>
    public sealed class GachaSummonStage : MonoBehaviour
    {
        private HubProgress _progress;
        private IReadOnlyList<HubCardDefinition> _catalog;
        private List<SummonResult> _cards;
        private bool[] _revealed;
        private Coroutine _revealRoutine;
        public IReadOnlyList<SummonResult> Cards => _cards;
        public bool HasPack => _cards != null;
        public bool IsUnsealed { get; private set; }
        public bool IsRevealing => _revealRoutine != null;
        public int RevealedCount { get; private set; }
        public bool IsComplete => HasPack && RevealedCount == _cards.Count;
        public event Action Changed;

        public void Initialize(HubProgress progress, IReadOnlyList<HubCardDefinition> catalog)
        {
            _progress = progress ?? throw new ArgumentNullException(nameof(progress));
            _catalog = catalog ?? throw new ArgumentNullException(nameof(catalog));
        }
        public bool PurchasePack()
        {
            if (_progress == null || HasPack || !_progress.TrySummon(_catalog, out var results)) return false;
            SetPack(results);
            return true;
        }
        public bool SummonAgain()
        {
            if (_progress == null || !IsComplete || IsRevealing || !_progress.TrySummon(_catalog, out var results)) return false;
            SetPack(results);
            return true;
        }
        private void SetPack(List<SummonResult> results)
        {
            _cards = results;
            _revealed = new bool[results.Count];
            IsUnsealed = false;
            RevealedCount = 0;
            Changed?.Invoke();
        }
        public bool Unseal()
        {
            if (!HasPack || IsUnsealed) return false;
            IsUnsealed = true;
            Changed?.Invoke();
            return true;
        }
        public bool IsRevealed(int index) => _revealed != null && index >= 0 && index < _revealed.Length && _revealed[index];
        public SummonRarity? HoverRarity(int index)
        {
            return IsUnsealed && _cards != null && index >= 0 && index < _cards.Count && !IsRevealed(index) ? _cards[index].Rarity : (SummonRarity?)null;
        }
        public bool Reveal(int index)
        {
            if (!IsUnsealed || _revealed == null || index < 0 || index >= _revealed.Length || _revealed[index]) return false;
            _revealed[index] = true;
            RevealedCount++;
            Changed?.Invoke();
            return true;
        }
        public void RevealAll()
        {
            if (!IsUnsealed || IsComplete || IsRevealing) return;
            if (!Application.isPlaying) { RevealAllImmediately(); return; }
            _revealRoutine = StartCoroutine(RevealCascade());
        }
        private IEnumerator RevealCascade()
        {
            for (int i = 0; i < _revealed.Length; i++)
            {
                if (Reveal(i)) yield return new WaitForSecondsRealtime(0.12f);
            }
            _revealRoutine = null;
            Changed?.Invoke();
        }
        public void RevealAllImmediately()
        {
            if (!IsUnsealed) return;
            for (int i = 0; i < _revealed.Length; i++) Reveal(i);
        }
        public bool Continue()
        {
            if (!IsComplete || IsRevealing) return false;
            _cards = null; _revealed = null; IsUnsealed = false; RevealedCount = 0;
            Changed?.Invoke();
            return true;
        }
        private void OnDisable()
        {
            if (_revealRoutine != null) { StopCoroutine(_revealRoutine); _revealRoutine = null; }
        }
    }
}
