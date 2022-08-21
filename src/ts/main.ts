while (true) {
  console.log(await Deno.core.opAsync("op_electrum_poll_events"));
}
