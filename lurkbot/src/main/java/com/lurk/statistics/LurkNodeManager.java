package com.lurk.statistics;

import java.util.Set;

/*
 * Node manager is in charge of all available nodes.
 */
public class LurkNodeManager {

    private final Set<LurkNode> nodes;

    public LurkNodeManager() {
        nodes = Set.of(
                new LurkNode("127.0.0.1", 8080),
                new LurkNode("164.92.219.216", 6996));
    }

    public Set<LurkNode> getVisibleNodes(long chatId) {
        // TODO: 
        //
        // There should be some whitelist of chat id's correspond
        // to some visible nodes. Initially, chat-id doesn't have any
        // available nodes. It should acquire some of them via some 
        // currently unimplemented process.
        //
        // For now, just return all nodes that we know about, so input
        // chat-id is unused. 
        return nodes;
    }
}
