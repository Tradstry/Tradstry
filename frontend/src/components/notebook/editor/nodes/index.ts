import { STANDARD_NODES } from "@tradstry/notebook-core";
import { GhostTextNode } from "./ghost-text-node";
import { LinkedTradeNode } from "./linked-trade-node";
import { NotebookImageNode } from "./notebook-image-node";
import { NotebookVideoNode } from "./notebook-video-node";

/** The shared node union, with this client's rendering subclasses substituted in. */
export const WEB_NODES = [
  ...STANDARD_NODES,
  NotebookImageNode,
  NotebookVideoNode,
  LinkedTradeNode,
  GhostTextNode,
];
