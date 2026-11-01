using System;
using System.Collections.Generic;
using Heav3ns.UI.Navigation;
using UnityEngine;

namespace Heav3ns.UI
{
    public enum GrimoireFaction { All, Alice, WhiteRabbit }
    public sealed class GrimoireDeckStudio : MonoBehaviour
    {
        private IReadOnlyList<HubCardDefinition> _catalog;
        private readonly List<HubCardDefinition> _filtered = new List<HubCardDefinition>();
        public IReadOnlyList<HubCardDefinition> FilteredCards => _filtered;
        public GrimoireFaction Faction { get; private set; }
        public int CostFilter { get; private set; }
        public bool FoilPreview { get; private set; }
        public int Page { get; private set; }
        public int PageCount => Mathf.Max(1, (_filtered.Count + 7) / 8);
        public event Action Changed;
        public void Initialize(IReadOnlyList<HubCardDefinition> catalog) { _catalog = catalog ?? throw new ArgumentNullException(nameof(catalog)); Refresh(); }
        public void SetFaction(GrimoireFaction faction) { Faction = faction; Page = 0; Refresh(); }
        // 0 means all; 6 includes cards costing 6 or more.
        public void SetCost(int cost) { CostFilter = Mathf.Clamp(cost, 0, 6); Page = 0; Refresh(); }
        public void SetFoil(bool enabled) { FoilPreview = enabled; Changed?.Invoke(); }
        public void SetPage(int page) { Page = Mathf.Clamp(page, 0, PageCount - 1); Changed?.Invoke(); }
        private void Refresh()
        {
            _filtered.Clear();
            if (_catalog == null) return;
            foreach (var card in _catalog)
            {
                if (card == null) continue;
                if (Faction == GrimoireFaction.Alice && card.deck != "Alice") continue;
                if (Faction == GrimoireFaction.WhiteRabbit && card.deck != "White Rabbit") continue;
                if (CostFilter != 0 && (CostFilter == 6 ? card.cost < 6 : card.cost != CostFilter)) continue;
                _filtered.Add(card);
            }
            Changed?.Invoke();
        }
        public int[] ManaCurve(GrimoireFaction deck)
        {
            var bars = new int[7];
            if (_catalog == null) return bars;
            foreach (var card in _catalog)
            {
                if (card == null || (deck == GrimoireFaction.Alice && card.deck != "Alice") || (deck == GrimoireFaction.WhiteRabbit && card.deck != "White Rabbit")) continue;
                bars[Mathf.Clamp(card.cost - 1, 0, 6)] += Mathf.Max(0, card.copies);
            }
            return bars;
        }
        public int[] ManaCurveForDeck(IReadOnlyList<string> ids)
        {
            var bars = new int[7];
            if (_catalog == null || ids == null) return bars;
            foreach (string id in ids)
                foreach (var card in _catalog)
                    if (card != null && card.id == id) { bars[Mathf.Clamp(card.cost - 1, 0, 6)]++; break; }
            return bars;
        }
    }
}
