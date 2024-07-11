package com.lurk.statistics;

import org.telegram.telegrambots.meta.api.methods.send.SendMessage;

public class LurkUtils {

    public static enum MessageParseMode {
        EMPTY,
        MARKDOWN,
        HTML
    };

    public static SendMessage buildMessageWithText(long chatId, String format, Object... args) {
        return buildMessageWithText(chatId, String.format(format, args));
    }

    public static SendMessage buildMessageWithText(long chatId, String text) {
        return buildMessageWithText(chatId, text, MessageParseMode.EMPTY);
    }

    public static SendMessage buildMessageWithText(long chatId, String text, MessageParseMode parseMode) {
        text = text.replace(".", "\\.");
        text = text.replace("-", "\\-");
        SendMessage sendMessage = SendMessage.builder()
                .chatId(chatId)
                .text(text)
                .build();
        switch (parseMode) {
            case MARKDOWN:
                sendMessage.setParseMode("MarkdownV2");
                break;

            case HTML:
                sendMessage.setParseMode("HTML");
                break;

            case EMPTY:
                break;

            default:
                break;
        }
        return sendMessage;
    }
}
