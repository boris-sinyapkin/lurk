package com.lurk.statistics;

import io.smallrye.config.ConfigMapping;

@ConfigMapping(prefix = "lurkbot")
public interface LurkBotConfiguration {

    String telegramBotToken();

}
