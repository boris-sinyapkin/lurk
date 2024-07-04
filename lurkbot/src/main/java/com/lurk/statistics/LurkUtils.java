package com.lurk.statistics;

import java.net.http.HttpResponse;
import org.telegram.telegrambots.meta.api.methods.send.SendMessage;

public class LurkUtils {

    public static String buildHealthcheckStatusString(LurkNode node, String status) {
        return String.format("%s: %s\n", node.toString(), status);
    }

    public static String buildHealthcheckStatusString(LurkNode node, HttpResponse<Void> httpResponse) {
        String nodeStatus = (httpResponse.statusCode() == 200)
                ? "is up and running"
                : "is died";
        return buildHealthcheckStatusString(node, nodeStatus);
    }

    public static SendMessage buildMessageWithText(long chatId, String format, Object... args) {
        return buildMessageWithText(chatId, String.format(format, args));
    }

    public static SendMessage buildMessageWithText(long chatId, String text) {
        return SendMessage.builder()
                .chatId(chatId)
                .text(text)
                .build();
    }
}
