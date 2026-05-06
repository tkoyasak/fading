import { getRandomEntry } from "../actions.tsx";
import { Viewer } from "../viewer.tsx";

export function ServerComponent() {
  return <Viewer initial={getRandomEntry()} fetchEntry={getRandomEntry} />;
}
