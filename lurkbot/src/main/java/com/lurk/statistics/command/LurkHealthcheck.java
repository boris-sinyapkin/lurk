package com.lurk.statistics.command;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Set;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.telegram.telegrambots.meta.api.methods.send.SendMessage;
import com.lurk.statistics.LurkHttpClientWrapper;
import com.lurk.statistics.LurkNode;
import com.lurk.statistics.LurkNodeManager;
import com.lurk.statistics.LurkUtils;

public class LurkHealthcheck implements LurkCommand {

    private static final Logger log = LoggerFactory.getLogger(LurkHealthcheck.class);

    private final LurkNodeManager nodeManager;
    private final LurkHttpClientWrapper httpClientWrapper;

    public LurkHealthcheck(LurkHttpClientWrapper httpClientWrapper, LurkNodeManager nodeManager) {
        this.nodeManager = nodeManager;
        this.httpClientWrapper = httpClientWrapper;
    }

    @Override
    public SendMessage execute(long chatId) {
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
            URI nodeUri = node.getHttpUri(path());
            HttpResponse<String> httpResponse;
            HttpRequest httpRequest =  LurkHttpClientWrapper.buildHttpGetRequest(nodeUri);

            try {
                log.debug("Sending GET request to {}", nodeUri);
                httpResponse = httpClientWrapper.send(httpRequest);
            } catch (IOException | InterruptedException e) {
                log.error("Exception thrown while sending GET request to {}", nodeUri, e);
                messageText.append(LurkUtils.buildHealthcheckStatusString(node, e.getMessage()));
                continue;
            }

            messageText.append(LurkUtils.buildHealthcheckStatusString(node, httpResponse));
        }

        return LurkUtils.buildMessageWithText(chatId, messageText.toString());
    }

    @Override
    public String path() {
        return "/healthcheck";
    }

}
