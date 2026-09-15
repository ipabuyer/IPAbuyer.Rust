import { create } from "zustand";

// 会话状态（对应原 C# SessionState）。null = 尚未查询。
type SessionState = {
  loggedIn: boolean | null;
  account: string | null;
  isMock: boolean;
  setSession: (loggedIn: boolean, account: string, isMock: boolean) => void;
  reset: () => void;
};

export const useSession = create<SessionState>((set) => ({
  loggedIn: null,
  account: null,
  isMock: false,
  setSession: (loggedIn, account, isMock) => set({ loggedIn, account, isMock }),
  reset: () => set({ loggedIn: false, account: null, isMock: false }),
}));
