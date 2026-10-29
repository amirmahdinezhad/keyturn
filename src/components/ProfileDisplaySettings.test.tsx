import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { makeState } from "../test/fixtures";
import type { AppSettings } from "../types";
const { save } = vi.hoisted(() => ({ save:vi.fn() }));
vi.mock("../api",()=>({setAppSettings:save,prettyError:(e:unknown)=>String(e)}));
import { ProfileDisplaySettings } from "./ProfileDisplaySettings";
const notify=vi.fn();
beforeEach(()=>{vi.clearAllMocks();save.mockImplementation(async(p:Partial<AppSettings>)=>makeState({app:{...makeState().app,...p}}));});
afterEach(cleanup);
function setup(){
  function Harness(){const [state,setState]=useState(makeState());return <ProfileDisplaySettings profiles={[{id:"work",name:"Work",active:true},{id:"personal",name:"Personal",active:false}]} app={state.app} onChanged={setState} notify={notify}/>;}
  return {user:userEvent.setup(),...render(<Harness/>)};
}
it("enables percent-only multi-profile display and saves chosen accounts",async()=>{
  const {user}=setup();
  await user.click(screen.getByRole("switch",{name:"Show multiple profiles"}));
  expect(screen.getByText("Percentage")).toBeInTheDocument();
  expect(screen.queryByText("Circles")).not.toBeInTheDocument();
  await user.click(screen.getByRole("checkbox",{name:/Work/}));
  expect(save).toHaveBeenLastCalledWith({multiProfile:expect.objectContaining({enabled:true,selectedIds:["work"]})});
  await user.click(screen.getByRole("switch",{name:"Show week usage"}));
  expect(save).toHaveBeenLastCalledWith({multiProfile:expect.objectContaining({showWeekly:true,selectedIds:["work"]})});
});
it("explains the cancellable restart when auto-switch is enabled",async()=>{
  const {user}=setup();await user.click(screen.getByRole("switch",{name:"Enable auto-switch"}));
  expect(save).toHaveBeenLastCalledWith({autoSwitchEnabled:true});
  expect(screen.getByText(/cancellable 30-second/)).toBeInTheDocument();
});
it("retains saved preferences when persistence fails",async()=>{
  save.mockRejectedValueOnce(new Error("Save failed"));const {user}=setup();
  await user.click(screen.getByRole("switch",{name:"Show multiple profiles"}));
  await waitFor(()=>expect(notify).toHaveBeenCalledWith(expect.stringContaining("Save failed"),"err"));
  expect(screen.getByRole("switch",{name:"Show multiple profiles"})).not.toBeChecked();
});
