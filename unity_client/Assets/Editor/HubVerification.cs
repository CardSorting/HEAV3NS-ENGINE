using System;
using Heav3ns.UI;
using Heav3ns.UI.Navigation;
using UnityEngine;

public static class HubVerification
{
    private static void Check(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    public static void Run()
    {
        var catalogAsset = Resources.Load<TextAsset>("Hub/CardCatalog");
        Check(catalogAsset != null, "Hub catalog must be available");
        var catalog = JsonUtility.FromJson<HubCardCatalog>(catalogAsset.text);
        Check(catalog != null && catalog.cards.Length == 44, "Catalog must contain 44 cards");
        int saved = 0, rolls = 0;
        var data = new HubSaveData { pullsSinceSsr = 9 };
        var progress = new HubProgress(data, _ => saved++, max => max == 100 ? (++rolls % 2 == 1 ? 0 : 99) : 0);
        progress.EnsureStarterDecks(catalog.cards);
        var host = new GameObject("Hub verification");
        try
        {
            var router = host.AddComponent<HubNavigationRouter>();
            Check(router.NavigateTo(HubScreenState.HubLobby), "Splash must enter hub");
            Check(!router.NavigateTo(HubScreenState.GachaSummonAltar), "Transition must block double navigation");
            router.AdvanceTransition(1);
            Check(router.NavigateTo(HubScreenState.GachaSummonAltar), "Hub must enter altar"); router.AdvanceTransition(1);
            Check(router.NavigateBack(), "Altar must return to hub"); router.AdvanceTransition(1);
            Check(router.NavigateTo(HubScreenState.GrimoireStudio), "Hub must enter studio"); router.AdvanceTransition(1);
            Check(router.NavigateBack(), "Studio must return to hub"); router.AdvanceTransition(1);
            Check(router.NavigateTo(HubScreenState.PreBattleDrawer), "Hub must enter pre-battle"); router.AdvanceTransition(1);
            Check(router.NavigateTo(HubScreenState.ActiveBattlefield), "Pre-battle must enter battle"); router.AdvanceTransition(1);
            Check(router.NavigateBack() && router.IsPauseOpen, "Battle back must open pause");
            Check(router.ReturnFromBattle() && router.Current == HubScreenState.HubLobby, "Battle return must enter lobby");
            router.AdvanceTransition(1);
            var carousel = host.AddComponent<EventCarouselWidget>(); carousel.Initialize(router); carousel.Activate();
            Check(router.Current == HubScreenState.GachaSummonAltar, "Event banner must open the advertised altar");
            router.AdvanceTransition(1); Check(router.NavigateBack(), "Event banner route must return to lobby"); router.AdvanceTransition(1);
            var hero = host.AddComponent<HeroCharacterShowcase>(); hero.Initialize(progress);
            Check(hero.StandeeShader != null, "Standee breathing shader must be available");
            string line = hero.Dialogue; hero.React(); Check(hero.Dialogue != line, "Attendant dialogue must cycle");
            hero.SwitchLeader(); Check(data.whiteRabbit, "Leader switch must persist");
            var studio = host.AddComponent<GrimoireDeckStudio>(); studio.Initialize(catalog.cards);
            studio.SetFaction(GrimoireFaction.WhiteRabbit); studio.SetCost(2);
            Check(studio.FilteredCards.Count > 0, "Rabbit cost-two filter must return cards");
            foreach (var card in studio.FilteredCards) Check(card.deck == "White Rabbit" && card.cost == 2, "Filters must match faction and cost");
            int total = 0; foreach (int n in studio.ManaCurveForDeck(progress.ActiveDeck)) total += n;
            Check(total == 30, "Mana curve must count the selected deck");
            var summon = host.AddComponent<GachaSummonStage>(); summon.Initialize(progress, catalog.cards);
            int keys = data.keys;
            Check(summon.PurchasePack() && data.keys == keys - 1 && data.pullsSinceSsr == 0, "Summon must debit a key and reset guaranteed pity");
            Check(summon.Unseal(), "Pack must unseal");
            bool rare = false, sr = false, ssr = false;
            for (int i = 0; i < summon.Cards.Count; i++)
            {
                var rarity = summon.HoverRarity(i);
                rare |= rarity == SummonRarity.Rare; sr |= rarity == SummonRarity.SuperRare; ssr |= rarity == SummonRarity.Ssr;
            }
            Check(rare && sr && ssr, "All rarity whispers must be available before reveal");
            summon.RevealAllImmediately(); Check(summon.IsComplete, "Reveal all must reveal all cards");
            Check(saved > 0, "Progress mutations must persist");
        }
        finally { UnityEngine.Object.DestroyImmediate(host); }
    }
}
