package com.lurk.statistics;

import java.net.http.HttpResponse;
import org.telegram.telegrambots.meta.api.methods.send.SendMessage;

public class LurkUtils {

    public static enum MessageParseMode {
        EMPTY,
        MARKDOWN,
        HTML
    };

    public static String buildHealthcheckStatusString(LurkNode node, String status) {
        return String.format("%s: %s\n", node.toString(), status);
    }

    public static String buildHealthcheckStatusString(LurkNode node, HttpResponse<?> httpResponse) {
        String nodeStatus = (httpResponse.statusCode() == 200)
                ? "is up and running"
                : "is died";
        return buildHealthcheckStatusString(node, nodeStatus);
    }

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
