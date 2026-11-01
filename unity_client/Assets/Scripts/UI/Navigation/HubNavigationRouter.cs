using System;
using System.Collections.Generic;
using UnityEngine;

namespace Heav3ns.UI.Navigation
{
    public enum HubScreenState
    {
        SplashScreen, HubLobby, PreBattleDrawer, GachaSummonAltar, GrimoireStudio, ActiveBattlefield
    }

    /// <summary>Single source of truth for hub navigation. Transitions are atomic and input is
    /// gated using unscaled time, so a paused game cannot leave the navigation locked.</summary>
    [DisallowMultipleComponent]
    public sealed class HubNavigationRouter : MonoBehaviour
    {
        [SerializeField, Range(0.35f, 0.55f)] private float transitionDuration = 0.45f;
        private readonly Stack<HubScreenState> _navigationHistory = new Stack<HubScreenState>();
        private float _transitionRemaining;
        public HubScreenState Current { get; private set; } = HubScreenState.SplashScreen;
        public bool IsTransitioning => _transitionRemaining > 0f;
        public bool IsPauseOpen { get; private set; }
        public int HistoryCount => _navigationHistory.Count;
        public event Action<HubScreenState, HubScreenState> ScreenChanged;
        public event Action<bool> PauseChanged;

        private void Update()
        {
            AdvanceTransition(Time.unscaledDeltaTime);
            HandleBackShortcut(UnityEngine.Input.GetKeyDown(KeyCode.Escape), UnityEngine.Input.GetMouseButtonDown(1));
        }

        public bool HandleBackShortcut(bool escapePressed, bool rightClickPressed)
        {
            return (escapePressed || rightClickPressed) && NavigateBack();
        }

        public void AdvanceTransition(float deltaTime)
        {
            _transitionRemaining = Mathf.Max(0f, _transitionRemaining - Mathf.Max(0f, deltaTime));
        }

        public bool NavigateTo(HubScreenState target)
        {
            if (IsTransitioning || IsPauseOpen || target == Current || !CanNavigate(Current, target)) return false;
            _navigationHistory.Push(Current);
            ChangeScreen(target);
            return true;
        }

        public bool NavigateBack()
        {
            if (IsTransitioning) return false;
            if (Current == HubScreenState.ActiveBattlefield)
            {
                SetPause(!IsPauseOpen);
                return true;
            }
            if (IsPauseOpen || _navigationHistory.Count == 0 || Current == HubScreenState.HubLobby) return false;
            ChangeScreen(_navigationHistory.Pop());
            return true;
        }

        public bool NavigateHome()
        {
            if (IsTransitioning || Current == HubScreenState.ActiveBattlefield) return false;
            if (Current == HubScreenState.HubLobby) return false;
            _navigationHistory.Clear();
            ChangeScreen(HubScreenState.HubLobby);
            return true;
        }

        public bool OpenSummonFromHeader()
        {
            if (IsTransitioning || IsPauseOpen || Current == HubScreenState.SplashScreen || Current == HubScreenState.ActiveBattlefield || Current == HubScreenState.GachaSummonAltar) return false;
            if (Current != HubScreenState.HubLobby)
            {
                _navigationHistory.Clear();
                _navigationHistory.Push(HubScreenState.HubLobby);
                ChangeScreen(HubScreenState.GachaSummonAltar);
                return true;
            }
            return NavigateTo(HubScreenState.GachaSummonAltar);
        }

        /// <summary>Use only after the battle's owner has stopped/reset the match.</summary>
        public bool ReturnFromBattle()
        {
            if (IsTransitioning || Current != HubScreenState.ActiveBattlefield) return false;
            SetPause(false);
            _navigationHistory.Clear();
            ChangeScreen(HubScreenState.HubLobby);
            return true;
        }

        public void SetPause(bool open)
        {
            if (Current != HubScreenState.ActiveBattlefield || IsPauseOpen == open) return;
            IsPauseOpen = open;
            PauseChanged?.Invoke(open);
        }

        private void ChangeScreen(HubScreenState target)
        {
            HubScreenState previous = Current;
            Current = target;
            _transitionRemaining = Mathf.Clamp(transitionDuration, 0.35f, 0.55f);
            ScreenChanged?.Invoke(previous, target);
        }

        private static bool CanNavigate(HubScreenState from, HubScreenState to)
        {
            switch (from)
            {
                case HubScreenState.SplashScreen: return to == HubScreenState.HubLobby;
                case HubScreenState.HubLobby:
                    return to == HubScreenState.PreBattleDrawer || to == HubScreenState.GachaSummonAltar || to == HubScreenState.GrimoireStudio || to == HubScreenState.SplashScreen;
                case HubScreenState.PreBattleDrawer: return to == HubScreenState.ActiveBattlefield;
                default: return false;
            }
        }
    }
}
