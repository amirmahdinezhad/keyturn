import { afterEach,beforeEach,expect,it,vi } from "vitest";
import {cleanup,render,screen} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
const api=vi.hoisted(()=>({codexAccountStatus:vi.fn(),codexAccountQuota:vi.fn(),importCodexProfileJson:vi.fn(),prettyError:(e:unknown)=>e instanceof Error?e.message:String(e)}));
vi.mock("../api",()=>api);
vi.mock("./CliProviders",()=>({formatAgo:()=>"just now"}));
import {CodexAccountModal} from "./CodexAccountModal";
beforeEach(()=>{vi.clearAllMocks();api.codexAccountStatus.mockResolvedValue({present:true,hasTokens:true,hasApiKey:false,email:"dev@example.com",lastRefresh:null,expiresAt:null});});
afterEach(cleanup);
const props={open:true,onClose:vi.fn(),onProfilesChanged:vi.fn(),notify:vi.fn()};
it("tests only the Codex account and surfaces usage-source errors",async()=>{
  const user=userEvent.setup();api.codexAccountQuota.mockResolvedValue({provider:"codex",error:"Session expired"});render(<CodexAccountModal {...props}/>);
  await screen.findByText("Signed in as dev@example.com");
  await user.click(screen.getByRole("button",{name:"Test connection"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Session expired");
  expect(api.codexAccountQuota).toHaveBeenCalledTimes(1);
});
it("saves pasted credentials natively and clears the text without activating",async()=>{
  const user=userEvent.setup();api.importCodexProfileJson.mockResolvedValue({profiles:[],activeProfileId:null,canImport:true,switchError:null});
  render(<CodexAccountModal {...props}/>);await screen.findByText("Connected");
  await user.click(screen.getByLabelText("Contents of auth.json"));
  await user.paste('{"tokens":{"synthetic":"test-only"}}');
  await user.click(screen.getByRole("button",{name:"Save credentials"}));
  expect(api.importCodexProfileJson).toHaveBeenCalledWith("",'{"tokens":{"synthetic":"test-only"}}');
  expect(screen.getByLabelText("Contents of auth.json")).toHaveValue("");
  expect(await screen.findByRole("status")).toHaveTextContent("Credentials saved as a profile");
});
it("rescans CLI metadata without importing credentials",async()=>{
  const user=userEvent.setup();render(<CodexAccountModal {...props}/>);await screen.findByText("Connected");
  await user.click(screen.getByRole("button",{name:"Rescan"}));
  expect(api.codexAccountStatus).toHaveBeenCalledTimes(2);expect(api.importCodexProfileJson).not.toHaveBeenCalled();
});
