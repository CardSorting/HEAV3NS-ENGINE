using System;
using System.Collections.Generic;
using UnityEngine;

namespace Heav3ns.UI.Navigation
{
    [Serializable] public sealed class HubCardDefinition
    {
        public string id, title, deck, type, text;
        public int copies, cost;
    }
    [Serializable] public sealed class HubCardCatalog { public HubCardDefinition[] cards; }
    [Serializable] public sealed class HubSaveData
    {
        public int version = 1;
        public int quartz = 4800;
        public int keys = 10;
        public int pullsSinceSsr;
        public int tea = 120;
        public long teaUpdatedAt;
        public bool whiteRabbit;
        public List<string> ownedCardIds = new List<string>();
        public bool starterCardsGranted;
        public bool decksInitialized;
        public List<string> aliceDeck = new List<string>();
        public List<string> rabbitDeck = new List<string>();
        public string dailyDay;
        public bool dailyVisitedGrimoire;
        public bool dailySummoned;
        public bool dailyClaimed;
        public List<int> claimedStoryStages = new List<int>();
    }
    public enum SummonRarity { Rare, SuperRare, Ssr }
    public sealed class SummonResult
    {
        public HubCardDefinition Card { get; }
        public SummonRarity Rarity { get; }
        public SummonResult(HubCardDefinition card, SummonRarity rarity) { Card = card; Rarity = rarity; }
    }

    /// <summary>Local prototype progression. Currency, pity and card grants are saved as one record.</summary>
    public sealed class HubProgress
    {
        public const string SaveKey = "heav3ns.hub.progress.v1";
        private readonly Action<HubSaveData> _save;
        private readonly Func<int, int> _random;
        public HubSaveData Data { get; }
        public event Action Changed;
        public HubProgress(HubSaveData data, Action<HubSaveData> save, Func<int, int> random)
        {
            Data = data ?? throw new ArgumentNullException(nameof(data));
            _save = save ?? throw new ArgumentNullException(nameof(save));
            _random = random ?? throw new ArgumentNullException(nameof(random));
            if (Data.ownedCardIds == null) Data.ownedCardIds = new List<string>();
        }
        public static HubProgress Load()
        {
            HubSaveData data = null;
            string json = PlayerPrefs.GetString(SaveKey, "");
            if (!string.IsNullOrEmpty(json))
            {
                try { data = JsonUtility.FromJson<HubSaveData>(json); }
                catch (ArgumentException) { Debug.LogWarning("Could not read local hub progress; starting a new profile."); }
            }
            return new HubProgress(data ?? new HubSaveData(), value => {
                PlayerPrefs.SetString(SaveKey, JsonUtility.ToJson(value));
                PlayerPrefs.Save();
            }, max => UnityEngine.Random.Range(0, max));
        }
        public void SetLeader(bool whiteRabbit)
        {
            if (Data.whiteRabbit == whiteRabbit) return;
            Data.whiteRabbit = whiteRabbit;
            Persist();
        }
        public List<string> ActiveDeck => Data.whiteRabbit ? Data.rabbitDeck : Data.aliceDeck;
        public void RefreshDaily(string day)
        {
            if (Data.dailyDay == day) return;
            Data.dailyDay = day; Data.dailyVisitedGrimoire = false; Data.dailySummoned = false; Data.dailyClaimed = false;
            Persist();
        }
        public void RecordGrimoireVisit() { if (!Data.dailyVisitedGrimoire) { Data.dailyVisitedGrimoire = true; Persist(); } }
        public bool ClaimDaily()
        {
            if (Data.dailyClaimed || !Data.dailyVisitedGrimoire || !Data.dailySummoned) return false;
            Data.dailyClaimed = true; Data.quartz += 160; Data.keys++; Persist(); return true;
        }
        public bool ClaimStoryVictory(int stage)
        {
            if (stage < 1 || stage > 3) return false;
            if (Data.claimedStoryStages == null) Data.claimedStoryStages = new List<int>();
            if (Data.claimedStoryStages.Contains(stage)) return false;
            Data.claimedStoryStages.Add(stage); Data.quartz += 160; if (stage == 3) Data.keys++;
            Persist(); return true;
        }
        public void EnsureStarterDecks(IReadOnlyList<HubCardDefinition> catalog)
        {
            if (Data.aliceDeck == null) Data.aliceDeck = new List<string>();
            if (Data.rabbitDeck == null) Data.rabbitDeck = new List<string>();
            bool grant = !Data.starterCardsGranted;
            bool initAlice = !Data.decksInitialized && Data.aliceDeck.Count == 0;
            bool initRabbit = !Data.decksInitialized && Data.rabbitDeck.Count == 0;
            foreach (var card in catalog)
            {
                if (card == null || string.IsNullOrEmpty(card.id)) continue;
                var deck = card.deck == "Alice" ? Data.aliceDeck : card.deck == "White Rabbit" ? Data.rabbitDeck : null;
                if (deck == null) continue;
                for (int i = 0; i < card.copies; i++)
                {
                    if (grant) Data.ownedCardIds.Add(card.id);
                    if (((card.deck == "Alice" && initAlice) || (card.deck == "White Rabbit" && initRabbit)) && deck.Count < 30) deck.Add(card.id);
                }
            }
            if (grant || !Data.decksInitialized) { Data.starterCardsGranted = true; Data.decksInitialized = true; Persist(); }
        }
        public bool AddToDeck(HubCardDefinition card)
        {
            if (card == null || card.deck != (Data.whiteRabbit ? "White Rabbit" : "Alice") || ActiveDeck.Count >= 30) return false;
            int inDeck = ActiveDeck.FindAll(id => id == card.id).Count;
            int owned = Data.ownedCardIds.FindAll(id => id == card.id).Count;
            if (inDeck >= Math.Min(card.copies, owned)) return false;
            ActiveDeck.Add(card.id); Persist(); return true;
        }
        public bool RemoveFromDeck(string id)
        {
            if (!ActiveDeck.Remove(id)) return false;
            Persist(); return true;
        }
        public void RefreshTea(long now)
        {
            if (Data.teaUpdatedAt <= 0 || now < Data.teaUpdatedAt)
            {
                Data.teaUpdatedAt = now;
                Persist();
                return;
            }
            if (Data.tea >= 120)
            {
                // A full meter does not bank regeneration for a later spend.
                Data.teaUpdatedAt = now;
                return;
            }
            int earned = (int)Math.Min(120L, (now - Data.teaUpdatedAt) / 300L);
            if (earned == 0) return;
            Data.tea = Math.Min(120, Data.tea + earned);
            Data.teaUpdatedAt = Data.tea == 120 ? now : Data.teaUpdatedAt + earned * 300L;
            Persist();
        }
        public bool TrySummon(IReadOnlyList<HubCardDefinition> catalog, out List<SummonResult> results)
        {
            results = null;
            if (catalog == null || catalog.Count == 0 || (Data.keys <= 0 && Data.quartz < 160)) return false;
            var validCards = new List<HubCardDefinition>();
            HubCardDefinition signature = null;
            foreach (var card in catalog)
            {
                if (card == null || string.IsNullOrEmpty(card.id)) continue;
                validCards.Add(card);
                if (card.id == "H3W-AL-008") signature = card;
            }
            if (validCards.Count == 0) return false;
            int nextPity = Data.pullsSinceSsr + 1;
            bool guaranteed = nextPity >= 10;
            bool ssr = guaranteed || _random(100) < 3;
            var generated = new List<SummonResult>(5);
            for (int i = 0; i < 5; i++)
            {
                SummonRarity rarity = i == 0 && ssr ? SummonRarity.Ssr : (_random(100) < 20 ? SummonRarity.SuperRare : SummonRarity.Rare);
                HubCardDefinition card = rarity == SummonRarity.Ssr && signature != null ? signature : validCards[_random(validCards.Count)];
                generated.Add(new SummonResult(card, rarity));
            }
            if (Data.keys > 0) Data.keys--; else Data.quartz -= 160;
            Data.pullsSinceSsr = ssr ? 0 : nextPity;
            foreach (var item in generated) Data.ownedCardIds.Add(item.Card.id);
            Data.dailySummoned = true;
            Persist();
            results = generated;
            return true;
        }
        private void Persist() { _save(Data); Changed?.Invoke(); }
    }
}
