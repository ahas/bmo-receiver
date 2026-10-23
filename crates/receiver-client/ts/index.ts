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
export type Answerers = Schemas["Answerers"];
export type Message = Schemas["Message"];
export type Pictures = Schemas["Pictures"];
export type Figures = Schemas["Figures"];
export type Window = Schemas["Window"];
export type Provider = Schemas["Provider"];
export type RoomProviders = Schemas["RoomProviders"];
export type ChooseProviders = Schemas["ChooseProviders"];
export type Notice = Schemas["Notice"];
export type Instruction = Schemas["Instruction"];
export type InstructionRequest = Schemas["InstructionRequest"];
export type SendRequest = Schemas["SendRequest"];
export type SendAnswer = Schemas["SendAnswer"];
export type Talk = Schemas["Talk"];
export type TalkAnswer = Schemas["TalkAnswer"];
export type CompanionRequest = Schemas["CompanionRequest"];
export type Companions = Schemas["Companions"];
export type PinnedRooms = Schemas["PinnedRooms"];
export type PinRooms = Schemas["PinRooms"];
export type PinRoom = Schemas["PinRoom"];
export type PublishedEvent = Schemas["PublishedEvent"];
export type Live = Schemas["Live"];
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

    /** `GET /rooms/{roomId}`. */
    async room(roomId: number): Promise<Room> {
      return answered(await client.GET("/rooms/{roomId}", { params: { path: { roomId } } }));
    },

    /** `GET /rooms/{roomId}/answerers`. */
    async answerers(roomId: number): Promise<Answerers> {
      return answered(await client.GET("/rooms/{roomId}/answerers", { params: { path: { roomId } } }));
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

    /** `GET /rooms/{roomId}/pictures`. */
    async pictures(
      roomId: number,
      query: NonNullable<paths["/rooms/{roomId}/pictures"]["get"]["parameters"]["query"]> = {},
    ): Promise<Pictures> {
      return answered(
        await client.GET("/rooms/{roomId}/pictures", { params: { path: { roomId }, query } }),
      );
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

    /** `GET /rooms/{roomId}/providers`. */
    async roomProviders(roomId: number): Promise<RoomProviders> {
      return answered(await client.GET("/rooms/{roomId}/providers", { params: { path: { roomId } } }));
    },

    /** `PUT /rooms/{roomId}/providers`. */
    async chooseProviders(roomId: number, body: ChooseProviders): Promise<RoomProviders> {
      return answered(await client.PUT("/rooms/{roomId}/providers", { params: { path: { roomId } }, body }));
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

    /** `POST /companions`. */
    async addCompanion(body: CompanionRequest): Promise<Companions> {
      return answered(await client.POST("/companions", { body }));
    },

    /** `DELETE /companions/{companionName}`. */
    async removeCompanion(companionName: string, by?: string): Promise<Companions> {
      return answered(
        await client.DELETE("/companions/{companionName}", {
          params: { path: { companionName }, query: by ? { by } : {} },
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

    /** `DELETE /pinned-rooms/{roomId}`. */
    async unpinRoom(roomId: number): Promise<PinnedRooms> {
      return answered(await client.DELETE("/pinned-rooms/{roomId}", { params: { path: { roomId } } }));
    },
  };
}

/** A client of the receiver, as `receiverClient` makes one. */
export type Receiver = ReturnType<typeof receiverClient>;
