import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {cleanup,render,screen} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
const api=vi.hoisted(()=>({claudeProfiles:vi.fn(),createClaudeProfile:vi.fn(),editClaudeProfile:vi.fn(),rescanClaudeProfile:vi.fn(),prettyError:(e:unknown)=>String(e)}));
vi.mock("../api",()=>api);
vi.mock("./CliProviders",()=>({CopyRow:({label,text}:{label:string;text:string})=><div>{label}: {text}</div>}));
import {ClaudeProfilesModal} from "./ClaudeProfilesModal";
const profile={id:"a",name:"Curious Otter",configDir:"/tmp/test-profile",createdAt:"2026-10-15T00:00:00Z",email:null,connected:false,subscription:null,quota:null,loginCommand:"CLAUDE_CONFIG_DIR='/tmp/test-profile' claude auth login",launchCommand:"CLAUDE_CONFIG_DIR='/tmp/test-profile' claude"};
const props={open:true,onClose:vi.fn(),onCodex:vi.fn(),appState:null,onAppChanged:vi.fn(),notify:vi.fn()};
beforeEach(()=>{vi.clearAllMocks();api.claudeProfiles.mockResolvedValue([]);});afterEach(cleanup);
it("creates an unnamed isolated profile and shows its CLI sign-in command",async()=>{
  const user=userEvent.setup();api.createClaudeProfile.mockResolvedValue([profile]);render(<ClaudeProfilesModal {...props}/>);
  await user.click(await screen.findByRole("button",{name:"Create new profile"}));
  await user.click(screen.getByRole("button",{name:"Create profile"}));
  expect(api.createClaudeProfile).toHaveBeenCalledWith("",false);
  expect(await screen.findByText(/Sign in to this profile:/)).toHaveTextContent(profile.loginCommand);
});
it("forgets profile metadata with an explicit preserved-login explanation",async()=>{
  const user=userEvent.setup();api.claudeProfiles.mockResolvedValue([profile]);api.editClaudeProfile.mockResolvedValue([]);render(<ClaudeProfilesModal {...props}/>);
  await user.click(await screen.findByRole("button",{name:/Curious Otter/}));
  await user.click(screen.getByRole("button",{name:"Remove profile"}));
  expect(screen.getByText(/login and session history stay/)).toBeInTheDocument();
  await user.click(screen.getByRole("button",{name:"Forget profile"}));
  expect(api.editClaudeProfile).toHaveBeenCalledWith("a",null);
});

it("opens the selected provider's create form with the drafted name", async()=>{
  render(<ClaudeProfilesModal {...props} initialCreate initialName="Research"/>);
  expect(await screen.findByLabelText("Profile name (optional)")).toHaveValue("Research");
});

it("hides confirmed expired subscriptions from the profile list", async () => {
  api.claudeProfiles.mockResolvedValue([
    profile,
    { ...profile, id: "ended", name: "Expired Work", subscription: { id: "claude", service: "Claude", status: "expired" } },
  ]);
  render(<ClaudeProfilesModal {...props} />);
  expect(await screen.findByRole("button", { name: /Curious Otter/ })).toBeInTheDocument();
  expect(screen.queryByText("Expired Work")).not.toBeInTheDocument();
});
