import { create } from "zustand";

// 会话状态（对应原 C# SessionState）。null = 启动后尚未查询。
// M1 占位：M2 由 auth_info / login / logout 命令更新。
type SessionState = {
  loggedIn: boolean | null;
  account: string | null;
  setSession: (loggedIn: boolean | null, account?: string | null) => void;
};

export const useSession = create<SessionState>((set) => ({
  loggedIn: null,
  account: null,
  setSession: (loggedIn, account = null) => set({ loggedIn, account }),
}));
