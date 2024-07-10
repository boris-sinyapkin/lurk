package com.lurk.statistics.command;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Optional;
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
    public String path() {
        return "/healthcheck";
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
        visibleNodes.forEach(node -> {
            HealthcheckResult result = doHealthcheck(node);
            messageText.append(result.toString() + "\n");
        });

        return LurkUtils.buildMessageWithText(chatId, messageText.toString());
    }

    private HealthcheckResult doHealthcheck(LurkNode node) {
        URI nodeUri = node.getHttpUri(path());
        HealthcheckResult result = new HealthcheckResult(node);

        HttpResponse<String> httpResponse;
        HttpRequest httpRequest = LurkHttpClientWrapper.buildHttpGetRequest(nodeUri);

        try {
            log.debug("Sending request to {}", nodeUri);
            httpResponse = httpClientWrapper.send(httpRequest);
        } catch (IOException | InterruptedException e) {
            log.error("Exception thrown while sending request to {}", nodeUri, e);
            result.setErrorMessage(e.getMessage());
            return result;
        }

        result.setHttpStatusCode(httpResponse.statusCode());
        return result;
    }

    private class HealthcheckResult {
        final LurkNode targetNode;

        Optional<Integer> httpStatusCode;
        Optional<String> errorMessage;

        HealthcheckResult(LurkNode targetNode) {
            this.targetNode = targetNode;
            this.httpStatusCode = Optional.empty();
            this.errorMessage = Optional.empty();
        }
    
        void setHttpStatusCode(Integer code) {
            httpStatusCode = Optional.of(code);
        }

        void setErrorMessage(String msg) {
            errorMessage = Optional.of(msg);
        }

        @Override
        public String toString() {
            StringBuilder str = new StringBuilder(targetNode.toString());
            if (httpStatusCode.isPresent()) {
                str.append(String.format("\n\tresponded with %d", httpStatusCode.get()));
            } else if (errorMessage.isPresent()) {
                str.append(String.format("\n\tfailed with error: %s", errorMessage.get()));
            }
            return str.toString();
        }
    }

}
