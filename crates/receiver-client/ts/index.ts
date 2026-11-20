// The chat receiver's fixed API as a TypeScript caller reaches it (docs/api.md, 「The wire」): the types
// `openapi-typescript` generates from docs/openapi.json, and one call for each route through `openapi-fetch`,
// each with the receiver's bearer token. A refusal is thrown as a `ReceiverError` carrying its status, code and
// sentence.

import createClient from "openapi-fetch";

import type { components, paths } from "./generated/openapi";

type Schemas = components["schemas"];

export type Health = Schemas["Health"];
export type Owner = Schemas["Owner"];
export type Room = Schemas["Room"];
export type RoomMode = Schemas["RoomMode"];
export type AddRoom = Schemas["AddRoom"];
export type AccountRoom = Schemas["AccountRoom"];
export type Answerers = Schemas["Answerers"];
export type Message = Schemas["Message"];
export type ChangeSpeaker = Schemas["ChangeSpeaker"];
export type Pictures = Schemas["Pictures"];
export type FileFill = Schemas["FileFill"];
export type FileFills = Schemas["FileFills"];
export type Avatar = Schemas["Avatar"];
export type Avatars = Schemas["Avatars"];
export type Figures = Schemas["Figures"];
export type Window = Schemas["Window"];
export type Provider = Schemas["Provider"];
export type RoomProviders = Schemas["RoomProviders"];
export type RoomMembers = Schemas["RoomMembers"];
export type RoomMember = Schemas["RoomMember"];
export type AgentRoom = Schemas["AgentRoom"];
export type RoomAccount = Schemas["RoomAccount"];
export type RoomAccounts = Schemas["RoomAccounts"];
export type Audience = Schemas["Audience"];
export type RoomAudience = Schemas["RoomAudience"];
export type LetIn = Schemas["LetIn"];
export type LetInSeveral = Schemas["LetInSeveral"];
export type LetInAccounts = Schemas["LetInAccounts"];
export type LetInUnit = Schemas["LetInUnit"];
export type LetInUnitAccounts = Schemas["LetInUnitAccounts"];
export type KeptFixedLine = Schemas["KeptFixedLine"];
export type AccountIdentity = Schemas["AccountIdentity"];
export type AccountNaming = Schemas["AccountNaming"];
export type AccountMemberships = Schemas["AccountMemberships"];
export type AccountWords = Schemas["AccountWords"];
export type AccountPresence = Schemas["AccountPresence"];
export type AccountTyping = Schemas["AccountTyping"];
export type ChooseProviders = Schemas["ChooseProviders"];
export type RoomRegistration = Schemas["RoomRegistration"];
export type ChangeRoomMode = Schemas["ChangeRoomMode"];
export type RoomArchive = Schemas["RoomArchive"];
export type RoomLeaders = Schemas["RoomLeaders"];
export type ChangeRoomArchive = Schemas["ChangeRoomArchive"];
export type RoomTone = Schemas["RoomTone"];
export type ChangeRoomTone = Schemas["ChangeRoomTone"];
export type ChangeRoomTitle = Schemas["ChangeRoomTitle"];
export type RoomTitle = Schemas["RoomTitle"];
export type Notice = Schemas["Notice"];
export type Instruction = Schemas["Instruction"];
export type InstructionRequest = Schemas["InstructionRequest"];
export type SendRequest = Schemas["SendRequest"];
export type SendAnswer = Schemas["SendAnswer"];
export type Talk = Schemas["Talk"];
export type TalkAnswer = Schemas["TalkAnswer"];
export type HandoffAsked = Schemas["HandoffAsked"];
export type HandoffWritten = Schemas["HandoffWritten"];
export type HandoverAsked = Schemas["HandoverAsked"];
export type HandoverPending = Schemas["HandoverPending"];
export type VoiceBriefing = Schemas["VoiceBriefing"];
export type VoiceBriefingSwitch = Schemas["VoiceBriefingSwitch"];
export type LineReading = Schemas["LineReading"];
export type SupervisorRequest = Schemas["SupervisorRequest"];
export type SupervisorAdded = Schemas["SupervisorAdded"];
export type AddedRoom = Schemas["AddedRoom"];
export type SupervisorReset = Schemas["SupervisorReset"];
export type SupervisorRoom = Schemas["SupervisorRoom"];
export type DefaultAnswerer = Schemas["DefaultAnswerer"];
export type RoomSupervisors = Schemas["RoomSupervisors"];
export type PinnedRooms = Schemas["PinnedRooms"];
export type PinRooms = Schemas["PinRooms"];
export type PinRoom = Schemas["PinRoom"];
export type PublishedEvent = Schemas["PublishedEvent"];
export type Live = Schemas["Live"];
export type RoundPipeline = Schemas["RoundPipeline"];
export type PipelineRound = Schemas["PipelineRound"];
export type InstructionChain = Schemas["InstructionChain"];
export type InstructionChainSet = Schemas["InstructionChainSet"];
export type InstructionText = Schemas["InstructionText"];
export type InstructionTextSet = Schemas["InstructionTextSet"];
export type Pins = Schemas["Pins"];
export type Sticker = Schemas["Sticker"];
export type Stickers = Schemas["Stickers"];

/** Who the console relay names as acting: the login's user id and its role (docs/api.md, 「Messages」). */
export interface Caller {
  key: string;
  role: "administrator" | "member";
}

function callerHeaders(caller: Caller) {
  return { "Owner-Key": caller.key, "Owner-Role": caller.role };
}
export type Page<T> = { items: T[]; more: boolean };

/** A refusal or failure the receiver answered, in the common error shape. */
export class ReceiverError extends Error {
  readonly status: number;
  readonly code: string | undefined;

  constructor(status: number, answered: unknown) {
    const error = answered as Partial<Schemas["WireError"]> | undefined;

    super(
      `the receiver answered ${status}: ${error?.error ?? "no error answer"}${error?.code ? ` (${error.code})` : ""}`,
    );
    this.name = "ReceiverError";
    this.status = status;
    this.code = error?.code;
  }
}

/** What a client is made with: the receiver's base address and the token every call carries. */
export interface ReceiverOptions {
  baseUrl: string;
  token: string;
}

// The route's answer, or its refusal thrown.
function answered<T>(result: { data?: T; error?: unknown; response: Response }): T {
  if (result.error !== undefined || !result.response.ok) {
    throw new ReceiverError(result.response.status, result.error);
  }

  return result.data as T;
}

/** The receiver, asked with one bearer token: one call for each route of the fixed API. */
export function receiverClient(options: ReceiverOptions) {
  const client = createClient<paths>({
    baseUrl: options.baseUrl.replace(/\/$/, ""),
    headers: { Authorization: `Bearer ${options.token}` },
  });

  return {
    /** `GET /health`. */
    async health(): Promise<Health> {
      return answered(await client.GET("/health"));
    },

    /** `GET /owner`. */
    async owner(): Promise<Owner> {
      return answered(await client.GET("/owner"));
    },

    /** `GET /rooms`. */
    async rooms(): Promise<Page<Room>> {
      return answered(await client.GET("/rooms"));
    },

    /** `POST /rooms`. */
    async addRoom(body: AddRoom): Promise<Room> {
      return answered(await client.POST("/rooms", { body }));
    },

    /** `GET /rooms/{roomId}`. */
    async room(roomId: number): Promise<Room> {
      return answered(await client.GET("/rooms/{roomId}", { params: { path: { roomId } } }));
    },

    /** `GET /rooms/{roomId}/answerers`. */
    async answerers(roomId: number): Promise<Answerers> {
      return answered(await client.GET("/rooms/{roomId}/answerers", { params: { path: { roomId } } }));
    },

    /** `GET /rooms/{roomId}/registration`, as the administrator the console names. */
    async roomRegistration(roomId: number, by: string): Promise<RoomRegistration> {
      return answered(
        await client.GET("/rooms/{roomId}/registration", {
          params: { path: { roomId }, query: { by } },
        }),
      );
    },

    /** `PUT /rooms/{roomId}/registration`, as the administrator the console names. */
    async changeRoomMode(roomId: number, by: string, body: ChangeRoomMode): Promise<RoomRegistration> {
      return answered(
        await client.PUT("/rooms/{roomId}/registration", {
          params: { path: { roomId }, query: { by } },
          body,
        }),
      );
    },

    /** `POST /rooms/{roomId}/file-fills`: a bounded pass over the room's placeholder rows. */
    async fillFiles(roomId: number, body: FileFill): Promise<FileFills> {
      return answered(
        await client.POST("/rooms/{roomId}/file-fills", {
          params: { path: { roomId } },
          body,
        }),
      );
    },

    /** `GET /rooms/{roomId}/leaders`. */
    async roomLeaders(roomId: number): Promise<RoomLeaders> {
      return answered(await client.GET("/rooms/{roomId}/leaders", { params: { path: { roomId } } }));
    },

    /** `GET /rooms/{roomId}/archive`. */
    async roomArchive(roomId: number): Promise<RoomArchive> {
      return answered(await client.GET("/rooms/{roomId}/archive", { params: { path: { roomId } } }));
    },

    /** `PUT /rooms/{roomId}/archive`, as the administrator or the room's leader the console names. */
    async changeRoomArchive(roomId: number, by: string, body: ChangeRoomArchive): Promise<RoomArchive> {
      return answered(
        await client.PUT("/rooms/{roomId}/archive", {
          params: { path: { roomId }, query: { by } },
          body,
        }),
      );
    },

    /** `PUT /rooms/{roomId}/title`, for a room on the record alone. */
    async changeRoomTitle(roomId: number, body: ChangeRoomTitle): Promise<RoomTitle> {
      return answered(await client.PUT("/rooms/{roomId}/title", { params: { path: { roomId } }, body }));
    },

    /** `GET /rooms/{roomId}/tone`. */
    async roomTone(roomId: number): Promise<RoomTone> {
      return answered(await client.GET("/rooms/{roomId}/tone", { params: { path: { roomId } } }));
    },

    /** `PUT /rooms/{roomId}/tone`, as the administrator the console names. */
    async changeRoomTone(roomId: number, by: string, body: ChangeRoomTone): Promise<RoomTone> {
      return answered(
        await client.PUT("/rooms/{roomId}/tone", {
          params: { path: { roomId }, query: { by } },
          body,
        }),
      );
    },

    /** `GET /rooms/{roomId}/messages`. */
    async messages(
      roomId: number,
      query: NonNullable<paths["/rooms/{roomId}/messages"]["get"]["parameters"]["query"]> = {},
    ): Promise<Page<Message>> {
      return answered(
        await client.GET("/rooms/{roomId}/messages", { params: { path: { roomId }, query } }),
      );
    },

    /** `GET /rooms/{roomId}/messages/{messageId}`. */
    async message(roomId: number, messageId: number): Promise<Message> {
      return answered(
        await client.GET("/rooms/{roomId}/messages/{messageId}", {
          params: { path: { roomId, messageId } },
        }),
      );
    },

    /** `PUT /rooms/{roomId}/messages/{messageId}/speaker`: a line under an agent's own number filed as that agent's. */
    async changeSpeaker(roomId: number, messageId: number, body: ChangeSpeaker): Promise<Message> {
      return answered(
        await client.PUT("/rooms/{roomId}/messages/{messageId}/speaker", {
          params: { path: { roomId, messageId } },
          body,
        }),
      );
    },

    /** `GET /rooms/{roomId}/pins`. */
    async pins(roomId: number): Promise<Pins> {
      return answered(await client.GET("/rooms/{roomId}/pins", { params: { path: { roomId } } }));
    },

    /** `PUT /rooms/{roomId}/messages/{messageId}/reactions/mine`, as the caller. */
    async react(caller: Caller, roomId: number, messageId: number, emoji: string): Promise<Message> {
      return answered(
        await client.PUT("/rooms/{roomId}/messages/{messageId}/reactions/mine", {
          params: { path: { roomId, messageId } },
          headers: callerHeaders(caller),
          body: { emoji },
        }),
      );
    },

    /** `DELETE /rooms/{roomId}/messages/{messageId}/reactions/mine`, as the caller. */
    async unreact(caller: Caller, roomId: number, messageId: number): Promise<Message> {
      return answered(
        await client.DELETE("/rooms/{roomId}/messages/{messageId}/reactions/mine", {
          params: { path: { roomId, messageId } },
          headers: callerHeaders(caller),
        }),
      );
    },

    /** `PUT /rooms/{roomId}/messages/{messageId}/pin`, as the caller. */
    async pin(caller: Caller, roomId: number, messageId: number, on: boolean): Promise<Message> {
      return answered(
        await client.PUT("/rooms/{roomId}/messages/{messageId}/pin", {
          params: { path: { roomId, messageId } },
          headers: callerHeaders(caller),
          body: { on },
        }),
      );
    },

    /** `PATCH /rooms/{roomId}/messages/{messageId}`, as the caller. */
    async editMessage(caller: Caller, roomId: number, messageId: number, text: string): Promise<Message> {
      return answered(
        await client.PATCH("/rooms/{roomId}/messages/{messageId}", {
          params: { path: { roomId, messageId } },
          headers: callerHeaders(caller),
          body: { text },
        }),
      );
    },

    /** `DELETE /rooms/{roomId}/messages/{messageId}`, as the caller. */
    async deleteMessage(caller: Caller, roomId: number, messageId: number): Promise<Message> {
      return answered(
        await client.DELETE("/rooms/{roomId}/messages/{messageId}", {
          params: { path: { roomId, messageId } },
          headers: callerHeaders(caller),
        }),
      );
    },

    /** `GET /stickers`. */
    async stickers(): Promise<Stickers> {
      return answered(await client.GET("/stickers"));
    },

    /** `POST /rooms/{roomId}/stickers`, as the caller. */
    async sendSticker(caller: Caller, roomId: number, emoji: string): Promise<Message> {
      return answered(
        await client.POST("/rooms/{roomId}/stickers", {
          params: { path: { roomId } },
          headers: callerHeaders(caller),
          body: { emoji },
        }),
      );
    },

    /** `GET /rooms/{roomId}/pictures`. */
    async pictures(
      roomId: number,
      query: NonNullable<paths["/rooms/{roomId}/pictures"]["get"]["parameters"]["query"]> = {},
    ): Promise<Pictures> {
      return answered(
        await client.GET("/rooms/{roomId}/pictures", { params: { path: { roomId }, query } }),
      );
    },

    /** `GET /avatars`. */
    async avatars(
      query: NonNullable<paths["/avatars"]["get"]["parameters"]["query"]> = {},
    ): Promise<Avatars> {
      return answered(await client.GET("/avatars", { params: { query } }));
    },

    /** `GET /rooms/{roomId}/figures`. */
    async figures(roomId: number, zone?: string): Promise<Figures> {
      return answered(
        await client.GET("/rooms/{roomId}/figures", {
          params: { path: { roomId }, query: zone ? { zone } : {} },
        }),
      );
    },

    /** `GET /rooms/{roomId}/window`. */
    async window(roomId: number): Promise<Window> {
      return answered(await client.GET("/rooms/{roomId}/window", { params: { path: { roomId } } }));
    },

    /** `GET /providers`. */
    async providers(): Promise<Page<Provider>> {
      return answered(await client.GET("/providers"));
    },

    /** `GET /providers/{key}/rooms`. */
    async accountRooms(
      key: string,
      query: NonNullable<paths["/providers/{key}/rooms"]["get"]["parameters"]["query"]> = {},
    ): Promise<Page<AccountRoom>> {
      return answered(await client.GET("/providers/{key}/rooms", { params: { path: { key }, query } }));
    },

    /** `GET /rooms/{roomId}/providers`. */
    async roomProviders(roomId: number): Promise<RoomProviders> {
      return answered(await client.GET("/rooms/{roomId}/providers", { params: { path: { roomId } } }));
    },

    /** `PUT /rooms/{roomId}/providers`. */
    async chooseProviders(roomId: number, body: ChooseProviders): Promise<RoomProviders> {
      return answered(await client.PUT("/rooms/{roomId}/providers", { params: { path: { roomId } }, body }));
    },

    /** `POST /rooms` with no chat service: an agent room on the record alone, opened by an account. */
    async addAgentRoom(body: AgentRoom): Promise<Room> {
      return answered(await client.POST("/rooms", { body }));
    },

    /** `GET /rooms/{roomId}/audience`: whether everyone in the room stands in the company of the agent's owner. */
    async roomAudience(roomId: number, agent: string): Promise<RoomAudience> {
      return answered(
        await client.GET("/rooms/{roomId}/audience", { params: { path: { roomId }, query: { agent } } }),
      );
    },

    /** `GET /rooms/{roomId}/accounts`. */
    async roomAccounts(roomId: number): Promise<RoomAccounts> {
      return answered(await client.GET("/rooms/{roomId}/accounts", { params: { path: { roomId } } }));
    },

    /** `PUT /rooms/{roomId}/accounts/{account}`. */
    async letIn(roomId: number, account: number, body: LetIn): Promise<RoomAccount> {
      return answered(
        await client.PUT("/rooms/{roomId}/accounts/{account}", { params: { path: { roomId, account } }, body }),
      );
    },

    /** `POST /rooms/{roomId}/accounts`: several accounts let in with one line. */
    async letInSeveral(roomId: number, body: LetInSeveral): Promise<LetInAccounts> {
      return answered(await client.POST("/rooms/{roomId}/accounts", { params: { path: { roomId } }, body }));
    },

    /** `POST /rooms/{roomId}/unit-accounts`: an organisation unit's people let in as they stand now, with one line. */
    async letInUnit(roomId: number, body: LetInUnit): Promise<LetInUnitAccounts> {
      return answered(await client.POST("/rooms/{roomId}/unit-accounts", { params: { path: { roomId } }, body }));
    },

    /** `DELETE /rooms/{roomId}/accounts/{account}`, the member itself as `by` where it leaves. */
    async takeOut(roomId: number, account: number, by: number): Promise<void> {
      answered(
        await client.DELETE("/rooms/{roomId}/accounts/{account}", {
          params: { path: { roomId, account }, query: { by } },
        }),
      );
    },

    /** `PUT /accounts/{account}/identity`, naming the identity where `body` gives a name. */
    async accountIdentity(account: number, body?: AccountNaming): Promise<AccountIdentity> {
      return answered(await client.PUT("/accounts/{account}/identity", { params: { path: { account } }, body }));
    },

    /** `GET /accounts/{account}/rooms`. */
    async accountMemberships(account: number): Promise<AccountMemberships> {
      return answered(await client.GET("/accounts/{account}/rooms", { params: { path: { account } } }));
    },

    /** `PUT /accounts/{account}/presence`. */
    async accountPresence(account: number, body: AccountPresence): Promise<void> {
      answered(await client.PUT("/accounts/{account}/presence", { params: { path: { account } }, body }));
    },

    /** `POST /rooms/{roomId}/typing`. */
    async accountTyping(roomId: number, body: AccountTyping): Promise<void> {
      answered(await client.POST("/rooms/{roomId}/typing", { params: { path: { roomId } }, body }));
    },

    /** `POST /rooms/{roomId}/words`. */
    async accountWords(roomId: number, body: AccountWords): Promise<Schemas["Created"]> {
      return answered(await client.POST("/rooms/{roomId}/words", { params: { path: { roomId } }, body }));
    },

    /** `GET /rooms/{roomId}/notices`. */
    async notices(
      roomId: number,
      query: NonNullable<paths["/rooms/{roomId}/notices"]["get"]["parameters"]["query"]> = {},
    ): Promise<Page<Notice>> {
      return answered(
        await client.GET("/rooms/{roomId}/notices", { params: { path: { roomId }, query } }),
      );
    },

    /** `POST /rooms/{roomId}/notices`. */
    async notice(roomId: number, body: unknown): Promise<Schemas["Created"]> {
      return answered(
        await client.POST("/rooms/{roomId}/notices", {
          params: { path: { roomId } },
          body: body as never,
        }),
      );
    },

    /** `POST /instructions`. */
    async queueInstruction(body: InstructionRequest): Promise<Schemas["Created"]> {
      return answered(await client.POST("/instructions", { body }));
    },

    /** `GET /instructions/{instructionId}`. */
    async instruction(instructionId: number): Promise<Instruction> {
      return answered(
        await client.GET("/instructions/{instructionId}", { params: { path: { instructionId } } }),
      );
    },

    /** `DELETE /instructions/{instructionId}`. */
    async withdrawInstruction(instructionId: number): Promise<void> {
      answered(
        await client.DELETE("/instructions/{instructionId}", { params: { path: { instructionId } } }),
      );
    },

    /** `POST /sends`. */
    async send(body: SendRequest): Promise<SendAnswer> {
      return answered(await client.POST("/sends", { body }));
    },

    /** `POST /talks`. */
    async talk(body: Talk): Promise<TalkAnswer> {
      return answered(await client.POST("/talks", { body }));
    },

    /** `POST /handoffs/{agentName}`: the file the agent's standing session wrote, or null where it wrote none. */
    async handOff(agentName: string, body: HandoffAsked): Promise<HandoffWritten | null> {
      const result = await client.POST("/handoffs/{agentName}", { params: { path: { agentName } }, body });

      return result.response.status === 204 ? (answered(result), null) : answered(result);
    },

    /** `POST /handovers/{agentName}`: the agent's one session hands over at its next turn. */
    async handOver(agentName: string, body: HandoverAsked): Promise<HandoverPending> {
      return answered(await client.POST("/handovers/{agentName}", { params: { path: { agentName } }, body }));
    },

    /** `GET /voice-briefing`. */
    async voiceBriefing(): Promise<VoiceBriefing> {
      return answered(await client.GET("/voice-briefing", {}));
    },

    /** `PUT /voice-briefing`, exactly one switch named. */
    async switchVoiceBriefing(body: VoiceBriefingSwitch): Promise<VoiceBriefing> {
      return answered(await client.PUT("/voice-briefing", { body }));
    },

    /** `POST /rooms/{roomId}/messages/{messageId}/reading`, for the account the console reads it for where it names one. */
    async readLine(roomId: number, messageId: number, account?: number): Promise<LineReading> {
      return answered(
        await client.POST("/rooms/{roomId}/messages/{messageId}/reading", {
          params: { path: { roomId, messageId }, query: account === undefined ? {} : { account } },
        }),
      );
    },

    /** `POST /events`. */
    async publishEvent(body: PublishedEvent): Promise<void> {
      answered(await client.POST("/events", { body }));
    },

    /** `GET /live`. */
    async live(): Promise<Live> {
      return answered(await client.GET("/live"));
    },

    /** `DELETE /rooms/{roomId}`, its preview where `dryRun`. */
    async removeRoom(roomId: number, by: string, dryRun = false): Promise<unknown> {
      return answered(
        await client.DELETE("/rooms/{roomId}", {
          params: { path: { roomId }, query: { by, dryRun } },
        }),
      );
    },

    /** `POST /supervisors`. */
    async addSupervisor(body: SupervisorRequest): Promise<SupervisorAdded> {
      return answered(await client.POST("/supervisors", { body }));
    },

    /** `DELETE /supervisors/{supervisorName}`. */
    async removeSupervisor(
      supervisorName: string,
      options: { by?: string; forgetBrain?: boolean; force?: boolean } = {},
    ): Promise<void> {
      answered(
        await client.DELETE("/supervisors/{supervisorName}", {
          params: { path: { supervisorName }, query: options },
        }),
      );
    },

    /** `POST /supervisors/{supervisorName}/resets`. */
    async resetSupervisor(supervisorName: string, body: SupervisorReset): Promise<unknown> {
      return answered(
        await client.POST("/supervisors/{supervisorName}/resets", { params: { path: { supervisorName } }, body }),
      );
    },

    /** `GET /supervisors/{supervisorName}/rooms`. */
    async supervisorRooms(supervisorName: string): Promise<Page<SupervisorRoom>> {
      return answered(
        await client.GET("/supervisors/{supervisorName}/rooms", { params: { path: { supervisorName } } }),
      );
    },

    /** `GET /default-answerer`. */
    async defaultAnswerer(): Promise<DefaultAnswerer> {
      return answered(await client.GET("/default-answerer"));
    },

    /** `PUT /default-answerer`. */
    async setDefaultAnswerer(body: DefaultAnswerer): Promise<DefaultAnswerer> {
      return answered(await client.PUT("/default-answerer", { body }));
    },

    /** `PUT /rooms/{roomId}/supervisors`. */
    async setRoomSupervisors(roomId: number, body: RoomSupervisors): Promise<RoomSupervisors> {
      return answered(await client.PUT("/rooms/{roomId}/supervisors", { params: { path: { roomId } }, body }));
    },

    /** `GET /rounds/pipeline`: what each named round's instruction pipeline wrote. */
    async roundPipeline(room: number, rounds: string[]): Promise<Page<RoundPipeline>> {
      return answered(
        await client.GET("/rounds/pipeline", { params: { query: { room, rounds: rounds.join(",") } } }),
      );
    },

    /** `GET /instruction-stages`: every chain, or with a supervisor named the one chain it follows. */
    async instructionChains(agentName?: string): Promise<Page<InstructionChain>> {
      return answered(
        await client.GET("/instruction-stages", {
          params: { query: agentName === undefined ? {} : { agentName } },
        }),
      );
    },

    /** `PUT /instruction-stages/{agentName}`: a supervisor's own chain. */
    async setInstructionChain(agentName: string, body: InstructionChainSet): Promise<InstructionChain> {
      return answered(
        await client.PUT("/instruction-stages/{agentName}", { params: { path: { agentName } }, body }),
      );
    },

    /** `DELETE /instruction-stages/{agentName}`: the supervisor follows the policy's chain again. */
    async removeInstructionChain(agentName: string): Promise<void> {
      answered(await client.DELETE("/instruction-stages/{agentName}", { params: { path: { agentName } } }));
    },

    /** `GET /instruction-texts`: a supervisor's catalogue, or the policy's alone where none is named. */
    async instructionTexts(agentName?: string): Promise<Page<InstructionText>> {
      return answered(
        await client.GET("/instruction-texts", {
          params: { query: agentName === undefined ? {} : { agentName } },
        }),
      );
    },

    /** `PUT /instruction-texts/{agentName}/{name}`. */
    async setInstructionText(agentName: string, name: string, body: InstructionTextSet): Promise<InstructionText> {
      return answered(
        await client.PUT("/instruction-texts/{agentName}/{name}", {
          params: { path: { agentName, name } },
          body,
        }),
      );
    },

    /** `DELETE /instruction-texts/{agentName}/{name}`. */
    async removeInstructionText(agentName: string, name: string): Promise<void> {
      answered(
        await client.DELETE("/instruction-texts/{agentName}/{name}", {
          params: { path: { agentName, name } },
        }),
      );
    },

    /** `PUT /pinned-rooms`. */
    async pinRooms(body: PinRooms): Promise<PinnedRooms> {
      return answered(await client.PUT("/pinned-rooms", { body }));
    },

    /** `POST /pinned-rooms`. */
    async pinRoom(body: PinRoom): Promise<PinnedRooms> {
      return answered(await client.POST("/pinned-rooms", { body }));
    },

    /** `DELETE /pinned-rooms/{roomId}`, for the account that unpins it where the console names one. */
    async unpinRoom(roomId: number, account?: number): Promise<PinnedRooms> {
      return answered(
        await client.DELETE("/pinned-rooms/{roomId}", {
          params: { path: { roomId }, query: account === undefined ? {} : { account } },
        }),
      );
    },
  };
}

/** A client of the receiver, as `receiverClient` makes one. */
export type Receiver = ReturnType<typeof receiverClient>;
