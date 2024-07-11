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
import com.lurk.statistics.LurkUtils.MessageParseMode;

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

        StringBuilder messageText = new StringBuilder("Nodes health status:\n\n");
        // Iterate over visible nodes, request their health status
        // and construct response message.
        visibleNodes.forEach(node -> {
            HealthcheckResult result = doHealthcheck(node);
            messageText.append(result.toMarkdownString() + "\n\n");
        });

        return LurkUtils.buildMessageWithText(chatId, messageText.toString(), MessageParseMode.MARKDOWN);
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
            this.errorMessage = Optional.empty();
            this.httpStatusCode = Optional.empty();
        }

        void setHttpStatusCode(Integer code) {
            httpStatusCode = Optional.of(code);
        }

        void setErrorMessage(String msg) {
            errorMessage = Optional.of(msg);
        }

        String toMarkdownString() {
            StringBuilder str = new StringBuilder();
            if (httpStatusCode.isPresent()) {
                int code = httpStatusCode.get();
                switch (code) {
                    case 200:
                        str.append("🟢 *%s*:\n- responded with *SUCCESS*".formatted(targetNode.toString()));
                        break;

                    default:
                        str.append("🟡 *%s*:\n- responded with %d HTTP status code".formatted(targetNode.toString(),
                                code));
                        break;
                }
            } else if (errorMessage.isPresent()) {
                str.append(
                        "🔴 *%s*:\n- *failed* with error: %s".formatted(targetNode.toString(), errorMessage.get()));
            }
            return str.toString();
        }
    }

}
