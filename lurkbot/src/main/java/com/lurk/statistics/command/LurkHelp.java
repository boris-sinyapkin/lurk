package com.lurk.statistics.command;

import org.telegram.telegrambots.meta.api.methods.send.SendMessage;

import com.lurk.statistics.LurkUtils;

public class LurkHelp implements LurkCommand {

    public LurkHelp() {}

    @Override
    public SendMessage execute(long chatId) {
        String helpText = """
                Available commands:
                    /help - view this information
                """;
        return LurkUtils.buildMessageWithText(chatId, helpText);
    }

    @Override
    public String path() {
        return "/help";
    }

}
