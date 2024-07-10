package com.lurk.statistics;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;

public class LurkHttpClientWrapper {

    private static final Duration HTTP_CLIENT_CONNECT_TIMEOUT = Duration.ofSeconds(1);
    private static final Duration HTTP_CLIENT_REQUEST_TIMEOUT = Duration.ofSeconds(1);

    private final HttpClient httpClient;

    public LurkHttpClientWrapper() {
        httpClient = HttpClient.newBuilder()
                .version(HttpClient.Version.HTTP_1_1)
                .connectTimeout(HTTP_CLIENT_CONNECT_TIMEOUT)
                .build();
    }

    public HttpResponse<String> send(HttpRequest httpRequest)
            throws IOException, InterruptedException {
        return httpClient.send(httpRequest, HttpResponse.BodyHandlers.ofString());
    }

    public static HttpRequest buildHttpGetRequest(URI uri) {
        return HttpRequest.newBuilder()
                .timeout(HTTP_CLIENT_REQUEST_TIMEOUT)
                .HEAD()
                .uri(uri)
                .build();
    }

}
