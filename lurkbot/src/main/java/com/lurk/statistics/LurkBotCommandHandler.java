package com.lurk.statistics;

import java.util.Set;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.meta.api.methods.send.SendMessage;
import com.lurk.statistics.command.LurkCommand;
import com.lurk.statistics.command.LurkHealthcheck;
import com.lurk.statistics.command.LurkHelp;

/*
 * Handles incoming Telegram commands. 
 */
public class LurkBotCommandHandler {

    private static final Logger log = LoggerFactory.getLogger(LurkBot.class);

    private final LurkHttpClientWrapper httpClientWrapper;
    private final LurkNodeManager nodeManager;
    private final Set<LurkCommand> commands;

    public LurkBotCommandHandler() {
        httpClientWrapper = new LurkHttpClientWrapper();
        nodeManager = new LurkNodeManager();
        commands = Set.of(
                new LurkHelp(),
                new LurkHealthcheck(httpClientWrapper, nodeManager));
    }

    public SendMessage handle(String path, long chatId) {
        LurkCommand command = findCommandByPath(path);
        if (command != null) {
            log.info("Executing command {} from chat_id={}", path, chatId);
            return command.execute(chatId);
        }

        log.error("Unknown command '{}' sent from chat_id={}", path, chatId);
        return LurkUtils.buildMessageWithText(chatId,
                "Unknown command '%s'. Try /help to see the list of available commands",
                path);
    }

    private LurkCommand findCommandByPath(String path) {
        for (LurkCommand command : commands) {
            if (command.path().equals(path)) {
                return command;
            }
        }
        return null;
    }

}