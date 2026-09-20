import { log, metado } from "@metado/runtime";

const GREETING = "hello from metado";

export default {
  boot() {
    return GREETING;
  },
  onMessage(message) {
    return `${GREETING} -> ${String(message)}`;
  },
};