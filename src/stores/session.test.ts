import { describe, expect, it } from "vitest";
import { useSession } from "./session";

// 用例按声明顺序执行：首个用例依赖 store 的模块级初始状态（未被 reset 触碰）
describe("useSession", () => {
  it("starts unqueried: loggedIn 为 null 表示尚未查询过登录状态", () => {
    const state = useSession.getState();
    expect(state.loggedIn).toBeNull();
    expect(state.account).toBeNull();
    expect(state.isMock).toBe(false);
  });

  it("setSession stores login state", () => {
    useSession.getState().reset();
    useSession.getState().setSession(true, "user@icloud.com", false);
    const state = useSession.getState();
    expect(state.loggedIn).toBe(true);
    expect(state.account).toBe("user@icloud.com");
    expect(state.isMock).toBe(false);
  });

  it("reset clears everything back to logged out", () => {
    useSession.getState().setSession(true, "test", true);
    expect(useSession.getState().isMock).toBe(true);
    useSession.getState().reset();
    const state = useSession.getState();
    expect(state.loggedIn).toBe(false);
    expect(state.account).toBeNull();
    expect(state.isMock).toBe(false);
  });
});
