package com.lurk.statistics;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.Set;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.meta.api.methods.send.SendMessage;

/*
 * Handles incoming Telegram commands. 
 */
public class LurkBotCommandHandler {

    private static final Logger log = LoggerFactory.getLogger(LurkBot.class);

    private static final Duration HTTP_CLIENT_CONNECT_TIMEOUT = Duration.ofSeconds(1);
    private static final Duration HTTP_CLIENT_REQUEST_TIMEOUT = Duration.ofSeconds(1);

    private final HttpClient httpClient;
    private final LurkNodeManager nodeManager;

    public LurkBotCommandHandler(LurkNodeManager nodeManager) {
        httpClient = HttpClient.newBuilder()
                .version(HttpClient.Version.HTTP_1_1)
                .connectTimeout(HTTP_CLIENT_CONNECT_TIMEOUT)
                .build();
        this.nodeManager = nodeManager;
    }

    public SendMessage handleCommand(String commandName, long chatId) {
        LurkBotCommand command = LurkBotCommand.fromLabel(commandName);
        log.info("Handling command {}", command);

        if (command == LurkBotCommand.HELP) {
            return handleHelp(chatId);
        } else if (command == LurkBotCommand.HEALTHCHECK) {
            return handleProxyHealthcheck(chatId);
        } else {
            return handleUnknownCommand(chatId, commandName);
        }
    }

    private SendMessage handleProxyHealthcheck(long chatId) {
        // Retrieve visible nodes for input chat_id.
        Set<LurkNode> visibleNodes = nodeManager.getVisibleNodes(chatId);
        log.debug("There's {} nodes visible for chat_id={}", visibleNodes.size(), chatId);

        // Bail out if chat-id doesn't have any nodes.
        if (visibleNodes.isEmpty()) {
            return LurkUtils.buildMessageWithText(chatId, "there's no visible nodes available");
        }

        StringBuilder messageText = new StringBuilder();
        // Iterate over visible nodes, request their health status
        // and construct response message.
        for (LurkNode node : visibleNodes) {
            URI nodeUri = node.getHttpUri("/healthcheck");
            HttpResponse<Void> httpResponse;
            HttpRequest httpRequest = buildHttpGetRequest(nodeUri);

            try {
                log.debug("Sending GET request to {}", nodeUri);
                httpResponse = httpClient.send(httpRequest, HttpResponse.BodyHandlers.discarding());
            } catch (IOException | InterruptedException e) {
                log.error("Exception thrown while sending GET request to {}", nodeUri, e);
                messageText.append(LurkUtils.buildHealthcheckStatusString(node, e.getMessage()));
                continue;
            }

            messageText.append(LurkUtils.buildHealthcheckStatusString(node, httpResponse));
        }

        return LurkUtils.buildMessageWithText(chatId, messageText.toString());
    }

    private SendMessage handleUnknownCommand(long chatId, String commandName) {
        log.error("Unknown command '{}' sent from chat_id={}", commandName, chatId);
        return LurkUtils.buildMessageWithText(chatId,
                "Unknown command '%s'. Try /help to see the list of available commands",
                commandName);
    }

    private SendMessage handleHelp(long chatId) {
        String helpText = """
                Available commands:
                    /help - view this information
                """;
        return LurkUtils.buildMessageWithText(chatId, helpText);
    }

    public HttpRequest buildHttpGetRequest(URI uri) {
        return HttpRequest.newBuilder()
                .timeout(HTTP_CLIENT_REQUEST_TIMEOUT)
                .GET()
                .uri(uri)
                .build();
    }

}
